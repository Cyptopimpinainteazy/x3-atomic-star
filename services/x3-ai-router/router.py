#!/usr/bin/env python3
"""Small OpenAI-compatible, budgeted model router. Standard library only."""
import argparse
import base64
import datetime as dt
import html
import json
import os
import re
import sqlite3
import threading
import time
import uuid
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CRITICAL = ("consensus", "finality", "settlement", "atomic", "cryptograph", "supply", "runtime upgrade", "slashing", "cross-vm")
MAX_BODY = 2_000_000
# A single completion may not ask for more output than this. The bound exists so
# the per-request reservation is an upper bound on what the provider can bill.
MAX_OUTPUT_TOKENS = 32_768
DEFAULT_OUTPUT_TOKENS = 4_096
# Endpoints OpenAI clients probe that this router deliberately does not
# implement. A 501 names the gap; a 404 reads as a wrong base URL.
UNSUPPORTED_PATHS = ("/v1/embeddings", "/v1/audio")


def responses_messages(request):
    """Translate a Responses API request into Chat Completions messages.

    Codex sends the Responses shape: a top-level `instructions` string plus an
    `input` list whose items are messages, function calls and function results.
    Only these three item kinds exist in what Codex sends, and each has a
    direct Chat Completions equivalent.
    """
    messages = []
    instructions = request.get("instructions")
    if isinstance(instructions, str) and instructions:
        messages.append({"role": "system", "content": instructions})
    for item in request.get("input") or []:
        if not isinstance(item, dict):
            continue
        kind = item.get("type", "message")
        if kind == "message":
            role = item.get("role", "user")
            if role in ("developer",):
                role = "system"
            if role not in ("system", "user", "assistant"):
                role = "user"
            text = "".join(part.get("text", "") for part in item.get("content") or [] if isinstance(part, dict))
            messages.append({"role": role, "content": text})
        elif kind == "function_call":
            messages.append({
                "role": "assistant",
                "content": None,
                "tool_calls": [{
                    "id": item.get("call_id", ""),
                    "type": "function",
                    "function": {"name": item.get("name", ""), "arguments": item.get("arguments") or "{}"},
                }],
            })
        elif kind == "function_call_output":
            messages.append({
                "role": "tool",
                "tool_call_id": item.get("call_id", ""),
                "content": str(item.get("output", "")),
            })
    return messages


def responses_tools(request, messages):
    """Flatten Responses function tools into the Chat Completions tool shape.

    A Responses tool carries `name`/`parameters` at the top level; a chat tool
    nests them under `function`. Namespaced tools are flattened, because the
    chat protocol has no namespace concept. `web_search` has no chat equivalent
    and is dropped rather than translated into something that would lie about
    what ran.
    """
    translated = []
    for tool in request.get("tools") or []:
        if not isinstance(tool, dict):
            continue
        kind = tool.get("type")
        if kind == "namespace":
            translated.extend(responses_tools({"tools": tool.get("tools")}, messages))
        elif kind == "function" and tool.get("name"):
            function = {"name": tool["name"], "parameters": tool.get("parameters") or {"type": "object", "properties": {}}}
            if tool.get("description"):
                function["description"] = tool["description"]
            translated.append({"type": "function", "function": function})
    if translated:
        choice = request.get("tool_choice")
        if choice in ("auto", "none", "required"):
            messages.append(("tool_choice", choice))
    return translated


def responses_request_to_chat(request):
    """A complete Chat Completions request for the provider loop to execute."""
    chat = {"messages": responses_messages(request)}
    tools = responses_tools(request, [])
    if tools:
        chat["tools"] = tools
        if request.get("tool_choice") in ("auto", "none", "required"):
            chat["tool_choice"] = request["tool_choice"]
        if isinstance(request.get("parallel_tool_calls"), bool):
            chat["parallel_tool_calls"] = request["parallel_tool_calls"]
    limit = request.get("max_output_tokens")
    if isinstance(limit, int) and limit > 0:
        chat["max_tokens"] = limit
    return chat


def chat_message_to_response_output(message, prefix):
    """Chat Completions message -> the Responses `output` list."""
    output = []
    content = message.get("content")
    if content:
        output.append({"id": prefix + "msg", "type": "message", "role": "assistant", "status": "completed",
                       "content": [{"type": "output_text", "text": content, "annotations": []}]})
    for index, call in enumerate(message.get("tool_calls") or []):
        function = call.get("function") or {}
        output.append({"id": f"{prefix}fc{index}", "type": "function_call", "status": "completed",
                       "call_id": call.get("id", ""), "name": function.get("name", ""),
                       "arguments": function.get("arguments") or "{}"})
    return output


def responses_envelope(response_id, model, output, usage=None, status="completed"):
    envelope = {"id": response_id, "object": "response", "created_at": int(time.time()),
                "status": status, "model": model, "output": output,
                "parallel_tool_calls": False, "tool_choice": "auto", "tools": []}
    if usage is not None:
        prompt = usage.get("prompt_tokens", 0) or 0
        completion = usage.get("completion_tokens", 0) or 0
        envelope["usage"] = {"input_tokens": prompt, "output_tokens": completion,
                             "total_tokens": usage.get("total_tokens", prompt + completion)}
    return envelope


class ResponsesStream:
    """Re-emit the Chat Completions SSE that `Router.stream` forwards as Responses events.

    `Router.stream` already owns provider selection, budgets, cooldowns and
    fallback, and it hands every upstream line to the `send` callback. This
    adapter is that callback: it parses chat chunks and writes the Responses
    event sequence Codex expects, so the budget logic is not duplicated.
    """

    def __init__(self, send, response_id, model):
        self.send = send
        self.response_id = response_id
        self.model = model
        self.message_started = False
        self.text = []
        self.calls = {}          # tool index -> {"id","name","arguments"}
        self.call_order = []
        self.finished = False

    def _emit(self, event):
        self.send(("data: " + json.dumps(event) + "\n\n").encode())

    def start(self):
        self._emit({"type": "response.created",
                    "response": responses_envelope(self.response_id, self.model, [], status="in_progress")})

    def _open_message(self):
        if not self.message_started:
            self.message_started = True
            self._emit({"type": "response.output_item.added", "output_index": 0,
                        "item": {"id": self.response_id + "msg", "type": "message", "role": "assistant",
                                 "status": "in_progress", "content": []}})

    def _open_call(self, index, call_id, name):
        self.message_started = True
        self._emit({"type": "response.output_item.added", "output_index": index + 1,
                    "item": {"id": f"{self.response_id}fc{index}", "type": "function_call",
                             "status": "in_progress", "call_id": call_id, "name": name, "arguments": ""}})

    def on_line(self, chunk):
        line = chunk.decode("utf-8", "replace").strip()
        if not line.startswith("data: "):
            return
        data = line[6:].strip()
        if data == "[DONE]" or not data:
            return
        try:
            event = json.loads(data)
        except ValueError:
            return
        choices = event.get("choices") or []
        if not choices:
            return
        delta = choices[0].get("delta") or {}
        text = delta.get("content")
        if text:
            self._open_message()
            self.text.append(text)
            self._emit({"type": "response.output_text.delta", "item_id": self.response_id + "msg",
                        "output_index": 0, "content_index": 0, "delta": text})
        for call in delta.get("tool_calls") or []:
            index = call.get("index", len(self.call_order))
            if index not in self.calls:
                self.calls[index] = {"id": call.get("id") or f"call_{index}", "name": "", "arguments": ""}
                self.call_order.append(index)
                function = call.get("function") or {}
                self.calls[index]["name"] = function.get("name", "")
                self._open_call(index, self.calls[index]["id"], self.calls[index]["name"])
            function = call.get("function") or {}
            if function.get("name"):
                self.calls[index]["name"] = function["name"]
            arguments = function.get("arguments")
            if arguments:
                self.calls[index]["arguments"] += arguments
                self._emit({"type": "response.function_call_arguments.delta",
                            "item_id": f"{self.response_id}fc{index}", "output_index": index + 1,
                            "delta": arguments})

    def finish(self):
        if self.finished:
            return
        self.finished = True
        output = []
        if self.text:
            text = "".join(self.text)
            self._emit({"type": "response.output_text.done", "item_id": self.response_id + "msg",
                        "output_index": 0, "content_index": 0, "text": text})
            item = {"id": self.response_id + "msg", "type": "message", "role": "assistant", "status": "completed",
                    "content": [{"type": "output_text", "text": text, "annotations": []}]}
            self._emit({"type": "response.output_item.done", "output_index": 0, "item": item})
            output.append(item)
        for index in self.call_order:
            call = self.calls[index]
            item = {"id": f"{self.response_id}fc{index}", "type": "function_call", "status": "completed",
                    "call_id": call["id"], "name": call["name"], "arguments": call["arguments"] or "{}"}
            self._emit({"type": "response.function_call_arguments.done", "item_id": item["id"],
                        "output_index": index + 1, "arguments": item["arguments"]})
            self._emit({"type": "response.output_item.done", "output_index": index + 1, "item": item})
            output.append(item)
        if not output:
            self._open_message()
            output = chat_message_to_response_output({"content": ""}, self.response_id)
        self._emit({"type": "response.completed",
                    "response": responses_envelope(self.response_id, self.model, output)})


def output_bound(request, config):
    """The largest completion this request can be billed for.

    The reservation is an upper bound, so it has to bound whichever output
    parameter the client actually set. Reading only `max_tokens` let a request
    that set `max_completion_tokens` instead be reserved at the 4096 default
    while the provider billed for whatever it asked for.
    """
    for key in ("max_tokens", "max_completion_tokens"):
        value = request.get(key)
        if type(value) is int:
            return value
    return config.get("default_max_output_tokens", DEFAULT_OUTPUT_TOKENS)


def request_error(request, config):
    """Reject a request whose cost the reservation would not bound.

    Two shapes defeat a per-request reservation: an output parameter the
    estimate does not read, and `n`/`best_of`, which multiply the completions
    the provider bills for while the estimate assumes exactly one.
    """
    for key in ("max_tokens", "max_completion_tokens"):
        value = request.get(key)
        if value is not None and (type(value) is not int or not 1 <= value <= config.get("max_output_tokens", MAX_OUTPUT_TOKENS)):
            return "Invalid " + key
    for key in ("n", "best_of"):
        value = request.get(key)
        if value is not None and value != 1:
            return key + " must be 1 when set: one reservation covers one completion"
    if request.get("model") is not None and not isinstance(request.get("model"), str):
        return "Invalid model"
    return None


def pricing_error(provider):
    """Fail closed for paid providers with missing or old price assumptions."""
    if not provider.get("api_key_env"):
        return None
    if provider.get("free_model"):
        if not provider.get("model", "").endswith(":free") or provider.get("input_usd_per_million", 0) != 0 or provider.get("output_usd_per_million", 0) != 0:
            return "free model must use a :free ID and zero prices"
    elif provider.get("input_usd_per_million", 0) <= 0 or provider.get("output_usd_per_million", 0) <= 0:
        return "configure positive token prices"
    try:
        checked = dt.date.fromisoformat(provider["pricing_checked_on"])
        age = (dt.datetime.now(dt.timezone.utc).date() - checked).days
        if age < 0 or age > provider.get("pricing_max_age_days", 30):
            return "refresh provider pricing"
    except (KeyError, TypeError, ValueError):
        return "configure pricing_checked_on (YYYY-MM-DD)"
    return None


def may_serve_critical(provider):
    """Whether a provider may be handed a critical request.

    Critical requests carry consensus and settlement code. A provider that
    authenticates against a third party (`api_key_env` set) may only see one
    when it is explicitly cleared with `critical_allowed`; being named in
    `budget_fallback` is not clearance, because the default fallback includes
    free cloud models whose operator logs prompts. A provider with no
    credentials is talking to a model on this machine, so it cannot leak the
    request to anyone and stays eligible — that is how a critical request keeps
    working when a paid provider is unavailable or over budget.
    """
    return bool(provider.get("critical_allowed", False)) or not provider.get("api_key_env")


class Router:
    def __init__(self, config, db_path):
        self.config = config
        self.db = sqlite3.connect(db_path, check_same_thread=False)
        self.lock = threading.Lock()
        self.context = threading.local()
        self.db.execute("CREATE TABLE IF NOT EXISTS usage (day TEXT, agent TEXT, provider TEXT, model TEXT, input_tokens INTEGER, output_tokens INTEGER, cost_usd REAL)")
        if "task_id" not in {row[1] for row in self.db.execute("PRAGMA table_info(usage)")}:
            self.db.execute("ALTER TABLE usage ADD COLUMN task_id TEXT")
        self.db.execute("CREATE TABLE IF NOT EXISTS tasks (id TEXT PRIMARY KEY, agent TEXT, revision TEXT, scope TEXT, requests INTEGER DEFAULT 0, elapsed_ms REAL DEFAULT 0, outcome TEXT DEFAULT 'pending', evidence TEXT, in_flight INTEGER DEFAULT 0)")
        self.db.execute("CREATE TABLE IF NOT EXISTS reservations (id TEXT PRIMARY KEY, day TEXT, agent TEXT, cost_usd REAL)")
        # A reservation is only deleted by `finish`, which runs in the request
        # thread. Without a timestamp there is no way to tell one that is still
        # in flight from one whose process died, so the day's `reserved_usd`
        # could only ever grow.
        if "created_at" not in {row[1] for row in self.db.execute("PRAGMA table_info(reservations)")}:
            self.db.execute("ALTER TABLE reservations ADD COLUMN created_at REAL")
        self.db.execute("CREATE TABLE IF NOT EXISTS provider_health (provider TEXT PRIMARY KEY, failures INTEGER DEFAULT 0, cooldown_until REAL DEFAULT 0, last_error TEXT, last_failure_at REAL)")
        self.db.commit()
        self.reconciled_orphans = 0
        self.reconcile_reservations()

    def choose(self, request):
        text = " ".join(str(m.get("content", "")) for m in request.get("messages", [])).lower()
        tier = "critical" if any(term in text for term in CRITICAL) else "routine"
        return tier, self.config["routes"][tier]

    def attempt_order(self, chain):
        """The providers to try, in order, for one request.

        `budget_fallback` is appended after the configured chain, so an
        exhausted budget or an unusable paid provider degrades the model
        instead of failing the request. Because it sits last, it is only
        reached when nothing better answered, and it can keep a critical
        request on this machine (a provider with no credentials cannot leak
        it) — but it never clears a third-party provider for critical work.
        See `may_serve_critical`.
        """
        order = list(chain)
        for name in self.config.get("budget_fallback", []):
            if name in self.config["providers"] and name not in order:
                order.append(name)
        return order

    def reserve(self, agent, estimate):
        day = dt.datetime.now(dt.timezone.utc).date().isoformat()
        with self.lock:
            self.db.execute("BEGIN IMMEDIATE")
            total = self.db.execute("SELECT COALESCE(SUM(cost_usd),0) FROM usage WHERE day=?", (day,)).fetchone()[0]
            total += self.db.execute("SELECT COALESCE(SUM(cost_usd),0) FROM reservations WHERE day=?", (day,)).fetchone()[0]
            spent = self.db.execute("SELECT COALESCE(SUM(cost_usd),0) FROM usage WHERE day=? AND agent=?", (day, agent)).fetchone()[0]
            spent += self.db.execute("SELECT COALESCE(SUM(cost_usd),0) FROM reservations WHERE day=? AND agent=?", (day, agent)).fetchone()[0]
            if total + estimate > self.config["daily_budget_usd"] or spent + estimate > self.config["agent_daily_budget_usd"]:
                self.db.commit()
                return None
            reservation = uuid.uuid4().hex
            self.db.execute("INSERT INTO reservations VALUES (?,?,?,?,?)", (reservation, day, agent, estimate, time.time()))
            self.db.commit()
            return reservation

    def finish(self, reservation, agent, provider=None, model=None, usage=None, cost=0):
        day = dt.datetime.now(dt.timezone.utc).date().isoformat()
        with self.lock:
            self.db.execute("BEGIN IMMEDIATE")
            self.db.execute("DELETE FROM reservations WHERE id=?", (reservation,))
            if provider is not None:
                self.db.execute("INSERT INTO usage (day,agent,provider,model,input_tokens,output_tokens,cost_usd,task_id) VALUES (?,?,?,?,?,?,?,?)", (day, agent, provider, model, usage.get("prompt_tokens", 0), usage.get("completion_tokens", 0), cost, getattr(self.context, "task_id", None)))
            self.db.commit()

    def stats(self):
        with self.lock:
            rows = self.db.execute("SELECT day,agent,provider,COUNT(*),ROUND(SUM(cost_usd),6) FROM usage GROUP BY day,agent,provider ORDER BY day DESC,agent").fetchall()
        return [{"day": d, "agent": a, "provider": p, "requests": n, "cost_usd": c} for d, a, p, n, c in rows]

    def reconcile_reservations(self, now=None):
        """Reclaim reservations left behind by a router that died mid-request.

        Nothing deletes a reservation except `finish`. If the process dies
        between reserving budget and calling the provider, the row is never
        removed: `reserved_usd` accumulates and the day's budget is consumed by
        requests that are not running. Startup calls this, so the next process
        to open the database starts from the truth.

        The TTL is far longer than any provider timeout, so a genuinely
        in-flight reservation is never reclaimed.
        """
        cutoff = (now if now is not None else time.time()) - self.config.get("reservation_ttl_seconds", 900)
        with self.lock:
            cursor = self.db.execute("DELETE FROM reservations WHERE created_at IS NULL OR created_at < ?", (cutoff,))
            self.db.commit()
            reclaimed = max(0, cursor.rowcount)
        self.reconciled_orphans += reclaimed
        return reclaimed

    def provider_cooldown(self, name, now=None):
        """Seconds this provider must be skipped for, or 0 when it is usable."""
        now = now if now is not None else time.time()
        with self.lock:
            row = self.db.execute("SELECT cooldown_until FROM provider_health WHERE provider=?", (name,)).fetchone()
        return max(0.0, (row[0] or 0) - now) if row else 0.0

    def note_provider_failure(self, name, error, retry_after=None):
        """Put a failing provider on cooldown so the next request skips it.

        Without this, a dead or rate-limited endpoint is retried by every
        request in turn: each one pays the timeout and the operators learn
        nothing until they read the logs. The delay doubles per consecutive
        failure and is capped, and an explicit `Retry-After` wins.
        """
        now = time.time()
        if retry_after is not None:
            try:
                delay = max(0.0, float(retry_after))
            except (TypeError, ValueError):
                delay = None
        else:
            delay = None
        with self.lock:
            self.db.execute("BEGIN IMMEDIATE")
            row = self.db.execute("SELECT failures FROM provider_health WHERE provider=?", (name,)).fetchone()
            failures = (row[0] or 0) + 1 if row else 1
            if delay is None:
                base = self.config.get("provider_cooldown_seconds", 60)
                delay = min(base * (2 ** min(failures - 1, 10)), self.config.get("provider_cooldown_max_seconds", 3600))
            self.db.execute(
                "INSERT INTO provider_health (provider,failures,cooldown_until,last_error,last_failure_at) VALUES (?,?,?,?,?) "
                "ON CONFLICT(provider) DO UPDATE SET failures=excluded.failures, cooldown_until=excluded.cooldown_until, "
                "last_error=excluded.last_error, last_failure_at=excluded.last_failure_at",
                (name, failures, now + delay, str(error)[:200], now))
            self.db.commit()
        return delay

    def note_provider_success(self, name):
        """A working provider starts its next request with a clean record."""
        with self.lock:
            self.db.execute("DELETE FROM provider_health WHERE provider=?", (name,))
            self.db.commit()

    def provider_health(self):
        now = time.time()
        with self.lock:
            rows = self.db.execute("SELECT provider,failures,cooldown_until,last_error FROM provider_health").fetchall()
        return [{"provider": p, "failures": f or 0, "cooldown_seconds": round(max(0.0, (u or 0) - now), 3), "last_error": e}
                for p, f, u, e in rows]

    def begin_task(self, task_id, agent, revision, scope):
        if not re.fullmatch(r"[A-Za-z0-9_-]{1,80}", task_id) or not re.fullmatch(r"[0-9a-f]{40}", revision) or scope != "router":
            raise ValueError("Expected task ID, 40-character revision, and router scope")
        with self.lock:
            self.db.execute("BEGIN IMMEDIATE")
            existing = self.db.execute("SELECT agent,revision,scope,outcome FROM tasks WHERE id=?", (task_id,)).fetchone()
            if existing and (existing[:3] != (agent, revision, scope) or existing[3] != "pending"):
                self.db.rollback()
                raise ValueError("Task binding differs or task is already finalized")
            self.db.execute("INSERT OR IGNORE INTO tasks (id,agent,revision,scope) VALUES (?,?,?,?)", (task_id, agent, revision, scope))
            self.db.execute("UPDATE tasks SET in_flight=in_flight+1 WHERE id=?", (task_id,))
            self.db.commit()
        self.context.task_id = task_id

    def end_task_request(self, elapsed_ms):
        task_id = getattr(self.context, "task_id", None)
        if task_id:
            with self.lock:
                self.db.execute("UPDATE tasks SET requests=requests+1,elapsed_ms=elapsed_ms+?,in_flight=in_flight-1 WHERE id=?", (elapsed_ms, task_id))
                self.db.commit()
        self.context.task_id = None

    def task_outcome(self, data):
        if not isinstance(data, dict):
            raise ValueError("Expected evidence object")
        checks = data.get("checks")
        if not isinstance(checks, list) or not checks or any(not isinstance(c, dict) or type(c.get("exit_code")) is not int or not re.fullmatch(r"[0-9a-f]{64}", c.get("output_sha256", "")) for c in checks):
            raise ValueError("Expected check exit codes and output SHA-256 digests")
        if data.get("scope") != "router" or [c.get("name") for c in checks] != ["router-tests"]:
            raise ValueError("Unsupported verification scope/checks")
        outcome = "checks_passed" if all(c["exit_code"] == 0 for c in checks) else "checks_failed"
        with self.lock:
            self.db.execute("BEGIN IMMEDIATE")
            task = self.db.execute("SELECT revision,scope,outcome,in_flight FROM tasks WHERE id=?", (data.get("task_id"),)).fetchone()
            if not task or task[:2] != (data.get("revision"), data.get("scope")) or task[2] != "pending" or task[3] != 0:
                self.db.rollback()
                raise ValueError("Unknown, mismatched, or finalized task")
            self.db.execute("UPDATE tasks SET outcome=?,evidence=? WHERE id=?", (outcome, json.dumps(checks), data["task_id"]))
            self.db.commit()
        return {"task_id": data["task_id"], "outcome": outcome, "scope": "router"}

    def task_stats(self):
        with self.lock:
            rows = self.db.execute("SELECT t.id,t.revision,t.scope,t.requests,t.elapsed_ms,t.outcome,COALESCE(SUM(u.cost_usd),0) FROM tasks t LEFT JOIN usage u ON u.task_id=t.id GROUP BY t.id ORDER BY t.rowid DESC LIMIT 100").fetchall()
        return [dict(zip(("task_id", "revision", "scope", "requests", "elapsed_ms", "outcome", "cost_usd"), row)) for row in rows]

    def learning_stats(self):
        with self.lock:
            rows = self.db.execute("SELECT u.provider,u.model,t.scope,COUNT(DISTINCT CASE WHEN t.outcome='checks_passed' THEN t.id END),COUNT(DISTINCT CASE WHEN t.outcome='checks_failed' THEN t.id END),SUM(CASE WHEN t.outcome IN ('checks_passed','checks_failed') THEN u.cost_usd ELSE 0 END) FROM usage u JOIN tasks t ON t.id=u.task_id GROUP BY u.provider,u.model,t.scope").fetchall()
        return [{"provider": p, "model": m, "scope": s, "passed_tasks": ok, "failed_tasks": bad,
                 "finalized_cost_usd": cost, "cost_per_passed_task_usd": cost / ok if ok else None,
                 "pass_rate": ok / (ok + bad) if ok + bad else None}
                for p, m, s, ok, bad, cost in rows]

    def snapshot(self):
        day = dt.datetime.now(dt.timezone.utc).date().isoformat()
        with self.lock:
            spent, requests, inputs, outputs = self.db.execute(
                "SELECT COALESCE(SUM(cost_usd),0),COUNT(*),COALESCE(SUM(input_tokens),0),COALESCE(SUM(output_tokens),0) FROM usage WHERE day=?", (day,)
            ).fetchone()
            reserved, inflight = self.db.execute(
                "SELECT COALESCE(SUM(cost_usd),0),COUNT(*) FROM reservations WHERE day=?", (day,)
            ).fetchone()
            rows = self.db.execute(
                "SELECT agent,provider,COUNT(*),SUM(cost_usd) FROM usage WHERE day=? GROUP BY agent,provider ORDER BY SUM(cost_usd) DESC", (day,)
            ).fetchall()
        return {"day": day, "spent_usd": spent, "reserved_usd": reserved, "requests": requests,
                "inflight": inflight, "input_tokens": inputs, "output_tokens": outputs,
                "reconciled_orphans": self.reconciled_orphans,
                "daily_budget_usd": self.config["daily_budget_usd"],
                "breakdown": [{"agent": a, "provider": p, "requests": n, "cost_usd": c} for a, p, n, c in rows]}


    def complete(self, request, agent):
        # UTF-8 JSON bytes conservatively bound visible input tokens; reject
        # oversized requests instead of trusting a configured estimate.
        if len(json.dumps(request, ensure_ascii=False).encode("utf-8")) > self.config["max_input_tokens"]:
            return 413, {"error": {"message": "Input exceeds configured budget bound"}}
        error = request_error(request, self.config)
        if error:
            return 400, {"error": {"message": error}}
        tier, chain = self.choose(request)
        failures = []
        budget_refused = False
        for name in self.attempt_order(chain):
            provider = self.config["providers"][name]
            if provider.get("enabled_env") and os.environ.get(provider["enabled_env"]) != "1":
                continue
            if tier == "critical" and not may_serve_critical(provider):
                failures.append(name + ": not cleared for critical work")
                continue
            cooldown = self.provider_cooldown(name)
            if cooldown > 0:
                failures.append(f"{name}: cooling down for {cooldown:.0f}s")
                continue
            model = provider["model"]
            price_in = provider.get("input_usd_per_million", 0)
            price_out = provider.get("output_usd_per_million", 0)
            error = pricing_error(provider)
            if error:
                failures.append(name + ": " + error)
                continue
            # Reserve against an upper-bound configured for each request before making the call.
            estimate = (output_bound(request, self.config) * price_out + self.config["max_input_tokens"] * price_in) / 1_000_000
            key = os.environ.get(provider.get("api_key_env", ""), "") if provider.get("api_key_env") else ""
            if provider.get("api_key_env") and not key:
                failures.append(name + ": credential unavailable")
                continue
            # A provider that cannot bill needs no reservation, and must not be
            # blocked by a budget that is already spent. Refusing a free local
            # model because a paid API is over its ceiling turns "stop spending"
            # into "stop working".
            if estimate <= 0:
                reservation = None
            else:
                reservation = self.reserve(agent, estimate)
                if reservation is None:
                    budget_refused = True
                    failures.append(name + ": daily budget exhausted")
                    continue
            payload = dict(request)
            payload["model"] = model
            payload["stream"] = False
            if provider.get("output_token_parameter") == "max_completion_tokens":
                payload["max_completion_tokens"] = payload.pop("max_tokens", 4096)
            headers = {"Content-Type": "application/json"}
            if key:
                headers["Authorization"] = "Bearer " + key
            url = provider["base_url"].rstrip("/") + "/chat/completions"
            try:
                call = urllib.request.Request(url, json.dumps(payload).encode(), headers, method="POST")
                with urllib.request.urlopen(call, timeout=provider.get("timeout_seconds", 120)) as response:
                    result = json.load(response)
                if not isinstance(result, dict) or "choices" not in result:
                    raise ValueError("Provider response lacks choices")
                usage = result.get("usage", {})
                cost = (usage.get("prompt_tokens", 0) * price_in + usage.get("completion_tokens", 0) * price_out) / 1_000_000 if usage else estimate
                self.note_provider_success(name)
                self.finish(reservation, agent, name, model, usage, cost)
                return 200, result
            except urllib.error.HTTPError as exc:
                # HTTPError is a subclass of URLError, so it has to be caught
                # first to read a rate-limit `Retry-After` instead of guessing.
                self.finish(reservation, agent)
                retry_after = exc.headers.get("Retry-After") if exc.headers else None
                self.note_provider_failure(name, "HTTP " + str(exc.code), retry_after)
                failures.append(f"{name}: HTTP {exc.code}")
            except (urllib.error.URLError, TimeoutError, ValueError) as exc:
                self.finish(reservation, agent)
                self.note_provider_failure(name, type(exc).__name__)
                failures.append(name + ": " + type(exc).__name__)
            except Exception:
                self.finish(reservation, agent)
                self.note_provider_failure(name, "unexpected error")
                raise
        if budget_refused:
            return 429, {"error": {"message": "Daily budget exhausted", "type": "budget_exceeded",
                                   "attempts": failures}}
        return 502, {"error": {"message": "No provider succeeded", "attempts": failures}}

    def stream(self, request, agent, start, send):
        if len(json.dumps(request, ensure_ascii=False).encode("utf-8")) > self.config["max_input_tokens"]:
            return 413, {"error": "Input exceeds configured budget bound"}
        error = request_error(request, self.config)
        if error:
            return 400, {"error": error}
        tier, chain = self.choose(request)
        failures = []
        budget_refused = False
        for name in self.attempt_order(chain):
            provider = self.config["providers"][name]
            if provider.get("enabled_env") and os.environ.get(provider["enabled_env"]) != "1":
                continue
            if tier == "critical" and not may_serve_critical(provider):
                failures.append(name + ": not cleared for critical work")
                continue
            cooldown = self.provider_cooldown(name)
            if cooldown > 0:
                failures.append(f"{name}: cooling down for {cooldown:.0f}s")
                continue
            price_in = provider.get("input_usd_per_million", 0)
            price_out = provider.get("output_usd_per_million", 0)
            error = pricing_error(provider)
            if error:
                failures.append(name + ": " + error)
                continue
            key = os.environ.get(provider.get("api_key_env", ""), "") if provider.get("api_key_env") else ""
            if provider.get("api_key_env") and not key:
                failures.append(name + ": credential unavailable")
                continue
            estimate = (output_bound(request, self.config) * price_out + self.config["max_input_tokens"] * price_in) / 1_000_000
            # A provider that cannot bill needs no reservation, and must not be
            # blocked by a budget that is already spent. Refusing a free local
            # model because a paid API is over its ceiling turns "stop spending"
            # into "stop working".
            if estimate <= 0:
                reservation = None
            else:
                reservation = self.reserve(agent, estimate)
                if reservation is None:
                    budget_refused = True
                    failures.append(name + ": daily budget exhausted")
                    continue
            payload = dict(request)
            payload["model"] = provider["model"]
            payload["stream"] = True
            payload["stream_options"] = {"include_usage": True}
            if provider.get("output_token_parameter") == "max_completion_tokens":
                payload["max_completion_tokens"] = payload.pop("max_tokens", 4096)
            headers = {"Content-Type": "application/json", "Accept": "text/event-stream"}
            if key:
                headers["Authorization"] = "Bearer " + key
            emitted = False
            usage = None
            try:
                call = urllib.request.Request(provider["base_url"].rstrip("/") + "/chat/completions",
                                              json.dumps(payload).encode(), headers, method="POST")
                with urllib.request.urlopen(call, timeout=provider.get("timeout_seconds", 120)) as response:
                    if "text/event-stream" not in response.headers.get("Content-Type", ""):
                        raise ValueError("Provider did not return SSE")
                    for line in response:
                        if len(line) > 1_000_000:
                            raise ValueError("Oversized SSE line")
                        if not line.startswith(b"data: "):
                            if emitted:
                                send(line)
                            continue
                        data = line[6:].strip()
                        if data != b"[DONE]":
                            event = json.loads(data)
                            if event.get("usage"):
                                usage = event["usage"]
                        if not emitted:
                            start()
                            emitted = True
                        send(line)
                if not emitted:
                    raise ValueError("Empty SSE response")
                cost = ((usage.get("prompt_tokens", 0) * price_in + usage.get("completion_tokens", 0) * price_out) / 1_000_000) if usage else estimate
                self.note_provider_success(name)
                self.finish(reservation, agent, name, provider["model"], usage or {}, cost)
                return None
            except urllib.error.HTTPError as exc:
                if emitted:
                    self.finish(reservation, agent, name, provider["model"], usage or {}, estimate)
                    return None
                self.finish(reservation, agent)
                retry_after = exc.headers.get("Retry-After") if exc.headers else None
                self.note_provider_failure(name, "HTTP " + str(exc.code), retry_after)
                failures.append(f"{name}: HTTP {exc.code}")
            except (urllib.error.URLError, TimeoutError, ValueError, OSError) as exc:
                if emitted:
                    self.finish(reservation, agent, name, provider["model"], usage or {}, estimate)
                    return None  # A partial stream cannot be retried with another model.
                self.finish(reservation, agent)
                self.note_provider_failure(name, type(exc).__name__)
                failures.append(name + ": " + type(exc).__name__)
            except Exception:
                self.finish(reservation, agent, name if emitted else None, provider["model"], usage or {}, estimate if emitted else 0)
                self.note_provider_failure(name, "unexpected error")
                raise
        if budget_refused:
            return 429, {"error": {"message": "Daily budget exhausted", "type": "budget_exceeded",
                                   "attempts": failures}}
        return 502, {"error": {"message": "No provider succeeded", "attempts": failures}}


def dashboard(snapshot):
    rows = "".join("<tr>" + "".join(f"<td>{html.escape(str(item[key]))}</td>" for key in ("agent", "provider", "requests", "cost_usd")) + "</tr>"
                   for item in snapshot["breakdown"])
    cells = "".join(f"<li><strong>{html.escape(key.replace('_', ' ').title())}:</strong> {html.escape(str(value))}</li>"
                    for key, value in snapshot.items() if key != "breakdown")
    return ("<!doctype html><html lang='en'><meta charset='utf-8'><meta http-equiv='refresh' content='15'>"
            "<meta name='viewport' content='width=device-width,initial-scale=1'><title>X3 AI router</title>"
            "<style>body{font:16px system-ui;background:#111827;color:#f9fafb;max-width:960px;margin:3rem auto;padding:1rem}"
            "table{border-collapse:collapse;width:100%}td,th{padding:.7rem;border-bottom:1px solid #4b5563;text-align:left}"
            "li{margin:.5rem 0}a{color:#fb923c}</style><h1>X3 AI router</h1><ul>" + cells +
            "</ul><h2>Today by agent and provider</h2><table><thead><tr><th>Agent</th><th>Provider</th>"
            "<th>Requests</th><th>USD</th></tr></thead><tbody>" + rows + "</tbody></table></html>")


def metrics(snapshot):
    fields = ("spent_usd", "reserved_usd", "requests", "inflight", "input_tokens", "output_tokens",
              "daily_budget_usd", "reconciled_orphans")
    return "".join(f"x3_ai_router_{name} {snapshot[name]}\n" for name in fields)


def handler_for(router):
    class Handler(BaseHTTPRequestHandler):
        def reply(self, status, data):
            body = json.dumps(data).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def authorized(self):
            secret = os.environ.get("X3_ROUTER_TOKEN")
            auth = self.headers.get("Authorization", "")
            return not secret or auth in ("Bearer " + secret, "Basic " + base64.b64encode(("x3:" + secret).encode()).decode())

        def raw(self, status, body, content_type):
            data = body.encode()
            self.send_response(status)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(data)

        def do_GET(self):
            if not self.authorized():
                self.send_response(401)
                self.send_header("WWW-Authenticate", 'Basic realm="X3 AI router"')
                self.end_headers()
                return
            if self.path == "/health":
                return self.reply(200, {"status": "ok"})
            if self.path == "/v1/usage":
                return self.reply(200, {"usage": router.stats()})
            if self.path == "/v1/tasks":
                return self.reply(200, {"tasks": router.task_stats()})
            if self.path == "/v1/learning":
                return self.reply(200, {"routing_mode": "fixed", "models": router.learning_stats()})
            if self.path == "/v1/dashboard":
                return self.raw(200, dashboard(router.snapshot()), "text/html; charset=utf-8")
            if self.path == "/metrics":
                return self.raw(200, metrics(router.snapshot()), "text/plain; version=0.0.4; charset=utf-8")
            if self.path == "/v1/models":
                return self.reply(200, {"object": "list", "data": [{"id": "x3-auto", "object": "model"}]})
            if self.path.startswith("/v1/models/"):
                if self.path.rsplit("/", 1)[-1] == "x3-auto":
                    return self.reply(200, {"id": "x3-auto", "object": "model"})
                return self.reply(404, {"error": {"message": "No such model"}})
            if self.path == "/v1/providers":
                return self.reply(200, {"providers": router.provider_health()})
            return self.reply(404, {"error": "Not found"})

        def stream_chunk(self, chunk):
            self.wfile.write(chunk)
            self.wfile.flush()

        def serve_responses(self, data, agent):
            """Serve `POST /v1/responses` by translating onto the chat path.

            Codex accepts only the Responses wire protocol for a custom
            provider, while every provider this router talks to speaks Chat
            Completions. Rather than duplicate provider selection, budgets,
            cooldowns and fallback, the request is translated and handed to
            `router.complete` / `router.stream`, and the answer is translated
            back.
            """
            if not isinstance(data.get("input"), list):
                return self.reply(400, {"error": {"message": "Expected an input list"}})
            chat = responses_request_to_chat(data)
            if not chat["messages"]:
                return self.reply(400, {"error": {"message": "Expected at least one input message"}})
            error = request_error(chat, router.config)
            if error:
                return self.reply(400, {"error": {"message": error}})

            response_id = "resp_" + uuid.uuid4().hex
            model = data.get("model") if isinstance(data.get("model"), str) else "x3-auto"

            if not data.get("stream", True):
                status, result = router.complete(chat, agent)
                if status != 200:
                    return self.reply(status, result)
                message = (result.get("choices") or [{}])[0].get("message") or {}
                envelope = responses_envelope(response_id, result.get("model", model),
                                              chat_message_to_response_output(message, response_id),
                                              result.get("usage"))
                return self.reply(200, envelope)

            stream = ResponsesStream(self.stream_chunk, response_id, model)

            def start():
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Cache-Control", "no-cache")
                self.send_header("Connection", "close")
                self.end_headers()
                stream.start()

            outcome = router.stream(chat, agent, start, stream.on_line)
            if outcome is not None:
                return self.reply(*outcome)
            stream.finish()
            self.close_connection = True
            return

        def do_POST(self):
            if self.path == "/v1/tasks/outcome":
                token = os.environ.get("X3_VERIFIER_TOKEN")
                if not token or token == os.environ.get("X3_ROUTER_TOKEN") or self.headers.get("Authorization") != "Bearer " + token:
                    return self.reply(403, {"error": "Independent verifier authorization required"})
                try:
                    size = int(self.headers.get("Content-Length", "0"))
                    if not 0 < size <= 65536:
                        return self.reply(413, {"error": "Invalid evidence size"})
                    return self.reply(200, router.task_outcome(json.loads(self.rfile.read(size))))
                except (ValueError, TypeError, KeyError):
                    return self.reply(400, {"error": "Invalid verification evidence"})
            if not self.authorized():
                return self.reply(401, {"error": "Unauthorized"})
            if any(self.path == path or self.path.startswith(path + "/") for path in UNSUPPORTED_PATHS):
                return self.reply(501, {"error": {"message": self.path + " is not implemented: this router speaks the Chat Completions API at /v1/chat/completions"}})
            if self.path not in ("/v1/chat/completions", "/v1/responses"):
                return self.reply(404, {"error": "Not found"})
            try:
                started = time.monotonic()
                size = int(self.headers.get("Content-Length", "0"))
                if size < 1 or size > MAX_BODY:
                    return self.reply(413, {"error": "Invalid request size"})
                data = json.loads(self.rfile.read(size))
                agent = self.headers.get("X-X3-Agent", "default")[:80]
                task_id = self.headers.get("X-X3-Task-ID")
                if task_id:
                    router.begin_task(task_id, agent, self.headers.get("X-X3-Revision", ""), self.headers.get("X-X3-Scope", "router"))
                if self.path == "/v1/responses":
                    return self.serve_responses(data, agent)
                if not isinstance(data.get("messages"), list) or not isinstance(data.get("stream", False), bool):
                    return self.reply(400, {"error": "Expected messages and boolean stream"})
                error = request_error(data, router.config)
                if error:
                    return self.reply(400, {"error": error})
                if data.get("stream"):
                    def start():
                        self.send_response(200)
                        self.send_header("Content-Type", "text/event-stream")
                        self.send_header("Cache-Control", "no-cache")
                        self.send_header("Connection", "close")
                        self.end_headers()

                    def send(chunk):
                        self.wfile.write(chunk)
                        self.wfile.flush()

                    outcome = router.stream(data, agent, start, send)
                    if outcome is not None:
                        return self.reply(*outcome)
                    self.close_connection = True
                    return
                status, result = router.complete(data, agent)
                return self.reply(status, result)
            except (ValueError, TypeError, KeyError):
                return self.reply(400, {"error": "Invalid request"})
            finally:
                router.end_task_request((time.monotonic() - started) * 1000)
    return Handler


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default=os.path.join(os.path.dirname(__file__), "config.json"))
    parser.add_argument("--db", default="x3-router.sqlite3")
    parser.add_argument("--port", type=int, default=11435)
    args = parser.parse_args()
    with open(args.config, encoding="utf-8") as source:
        config = json.load(source)
    server = ThreadingHTTPServer(("127.0.0.1", args.port), handler_for(Router(config, args.db)))
    server.serve_forever()


if __name__ == "__main__":
    main()
