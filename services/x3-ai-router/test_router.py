import importlib.util
import json
import os
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

spec = importlib.util.spec_from_file_location("x3_router", Path(__file__).with_name("router.py"))
router_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(router_module)


class Provider(BaseHTTPRequestHandler):
    requests = []

    def do_POST(self):
        data = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        self.requests.append(data)
        if data.get("stream"):
            chunks = [b'data: {"choices":[{"delta":{"content":"ok"}}]}\n\n',
                      b'data: {"choices":[],"usage":{"prompt_tokens":10,"completion_tokens":5}}\n\n',
                      b'data: [DONE]\n\n']
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            for chunk in chunks:
                self.wfile.write(chunk)
                self.wfile.flush()
            return
        body = json.dumps({"choices": [{"message": {"role": "assistant", "content": "ok"}}], "usage": {"prompt_tokens": 10, "completion_tokens": 5}}).encode()
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


class NativeResponsesProvider(BaseHTTPRequestHandler):
    requests = []
    paths = []
    fail = False
    truncate_stream = False
    terminal_failure = False

    def do_POST(self):
        data = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        type(self).requests.append(data)
        type(self).paths.append(self.path)
        if type(self).fail:
            body = json.dumps({"error": {"message": "native provider failure"}}).encode()
            self.send_response(503)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path != "/responses":
            body = json.dumps({"error": {"message": "wrong endpoint"}}).encode()
            self.send_response(404)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if data.get("stream"):
            if type(self).terminal_failure:
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.end_headers()
                self.wfile.write(b'event: response.failed\ndata: {"type":"response.failed","sequence_number":0,"response":{"id":"resp_native","object":"response","status":"failed","model":"deepseek-flash","output":[],"error":{"code":"provider_failed","message":"native terminal failure"},"usage":{"input_tokens":10,"output_tokens":0,"total_tokens":10}}}\n\n')
                self.wfile.flush()
                return
            if type(self).truncate_stream:
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.end_headers()
                self.wfile.write(b'event: response.created\ndata: {"type":"response.created","sequence_number":0,"response":{"id":"resp_native","object":"response","status":"in_progress","model":"deepseek-flash","output":[]}}\n\n')
                self.wfile.flush()
                return
            events = [
                b'event: response.created\ndata: {"type":"response.created","sequence_number":0,"response":{"id":"resp_native","object":"response","status":"in_progress","model":"deepseek-flash","output":[]}}\n\n',
                b'event: response.output_item.added\ndata: {"type":"response.output_item.added","sequence_number":1,"output_index":0,"item":{"id":"msg_native","type":"message","role":"assistant","status":"in_progress","content":[]}}\n\n',
                b'event: response.output_text.delta\ndata: {"type":"response.output_text.delta","sequence_number":2,"item_id":"msg_native","output_index":0,"content_index":0,"delta":"native ok"}\n\n',
                b'event: response.completed\ndata: {"type":"response.completed","sequence_number":3,"response":{"id":"resp_native","object":"response","status":"completed","model":"deepseek-flash","output":[{"id":"msg_native","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"native ok","annotations":[]}]}],"usage":{"input_tokens":10,"output_tokens":5,"total_tokens":15}}}\n\n',
            ]
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            for event in events:
                self.wfile.write(event)
                self.wfile.flush()
            return
        body = json.dumps({
            "id": "resp_native",
            "object": "response",
            "status": "failed" if type(self).terminal_failure else "completed",
            "model": "deepseek-flash",
            "error": {"code": "provider_failed", "message": "native terminal failure"} if type(self).terminal_failure else None,
            "output": [] if type(self).terminal_failure else [{
                "id": "msg_native",
                "type": "message",
                "role": "assistant",
                "status": "completed",
                "content": [{"type": "output_text", "text": "native ok", "annotations": []}],
            }],
            "usage": {"input_tokens": 10, "output_tokens": 0 if type(self).terminal_failure else 5, "total_tokens": 10 if type(self).terminal_failure else 15},
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass


class RouterTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        Provider.requests = []
        NativeResponsesProvider.requests = []
        NativeResponsesProvider.paths = []
        NativeResponsesProvider.fail = False
        NativeResponsesProvider.truncate_stream = False
        NativeResponsesProvider.terminal_failure = False
        self.upstream = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
        self.upstream_thread = threading.Thread(target=self.upstream.serve_forever, daemon=True)
        self.upstream_thread.start()
        self.config = {
            "daily_budget_usd": 0.01, "agent_daily_budget_usd": 0.01,
            "max_input_tokens": 1000, "routes": {"routine": ["down", "up"], "critical": ["up"]},
            "providers": {
                "down": {"base_url": "http://127.0.0.1:1/v1", "model": "down", "input_usd_per_million": 1, "output_usd_per_million": 1},
                "up": {"base_url": f"http://127.0.0.1:{self.upstream.server_port}/v1", "model": "up", "critical_allowed": True,
                       "input_usd_per_million": 1, "output_usd_per_million": 1}
            }
        }
        self.router = router_module.Router(self.config, self.tmp.name + "/usage.db")

    def tearDown(self):
        self.upstream.shutdown()
        self.upstream.server_close()
        self.tmp.cleanup()

    def test_fallback_and_accounting(self):
        status, response = self.router.complete({"messages": [{"role": "user", "content": "format this"}], "max_tokens": 10}, "alice")
        self.assertEqual(status, 200)
        self.assertEqual(response["choices"][0]["message"]["content"], "ok")
        self.assertEqual(Provider.requests[0]["model"], "up")
        self.assertEqual(self.router.stats()[0]["cost_usd"], 0.000015)

    def test_critical_skips_unapproved_provider(self):
        self.assertEqual(self.router.choose({"messages": [{"content": "atomic settlement"}]})[0], "critical")
        status, _ = self.router.complete({"messages": [{"content": "atomic settlement"}], "max_tokens": 10}, "alice")
        self.assertEqual(status, 200)
        self.assertEqual(len(Provider.requests), 1)

    def test_concurrent_reservations_and_input_bound(self):
        self.config["daily_budget_usd"] = 0.00101
        self.config["agent_daily_budget_usd"] = 0.00101
        with ThreadPoolExecutor(max_workers=8) as pool:
            ids = list(pool.map(lambda _: self.router.reserve("alice", 0.001), range(8)))
        self.assertEqual(sum(x is not None for x in ids), 1)
        self.router.finish(next(x for x in ids if x), "alice")
        self.assertIsNotNone(self.router.reserve("alice", 0.001))
        status, _ = self.router.complete({"messages": [{"content": "z" * 1100}]}, "alice")
        self.assertEqual(status, 413)

    def test_stream_fallback_and_usage(self):
        chunks = []
        started = []
        result = self.router.stream({"messages": [{"role": "user", "content": "format"}], "max_tokens": 10},
                                    "alice", lambda: started.append(True), chunks.append)
        self.assertIsNone(result)
        self.assertEqual(started, [True])
        self.assertIn(b"data: [DONE]", b"".join(chunks))
        self.assertTrue(Provider.requests[0]["stream_options"]["include_usage"])
        self.assertEqual(self.router.stats()[0]["cost_usd"], 0.000015)

    def test_http_stream_endpoint(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), router_module.handler_for(self.router))
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        try:
            data = json.dumps({"model": "x3-auto", "messages": [{"role": "user", "content": "format"}],
                               "max_tokens": 10, "stream": True}).encode()
            request = urllib.request.Request(f"http://127.0.0.1:{server.server_port}/v1/chat/completions",
                                             data, {"Content-Type": "application/json", "X-X3-Agent": "alice"})
            with urllib.request.urlopen(request) as response:
                self.assertEqual(response.headers["Content-Type"], "text/event-stream")
                self.assertIn(b"data: [DONE]", response.read())
            self.assertEqual(self.router.stats()[0]["cost_usd"], 0.000015)
        finally:
            server.shutdown()
            server.server_close()

    def test_dashboard_metrics_and_auth(self):
        self.router.finish(self.router.reserve("<script>", 0.001), "<script>", "up", "up",
                           {"prompt_tokens": 10, "completion_tokens": 5}, 0.000015)
        server = ThreadingHTTPServer(("127.0.0.1", 0), router_module.handler_for(self.router))
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        previous = os.environ.get("X3_ROUTER_TOKEN")
        os.environ["X3_ROUTER_TOKEN"] = "test-secret"
        try:
            url = f"http://127.0.0.1:{server.server_port}"
            with self.assertRaises(urllib.error.HTTPError) as rejected:
                urllib.request.urlopen(url + "/v1/dashboard")
            self.assertEqual(rejected.exception.code, 401)
            headers = {"Authorization": "Bearer test-secret"}
            with urllib.request.urlopen(urllib.request.Request(url + "/v1/dashboard", headers=headers)) as response:
                page = response.read().decode()
                self.assertIn("&lt;script&gt;", page)
                self.assertNotIn("<script>", page)
            with urllib.request.urlopen(urllib.request.Request(url + "/metrics", headers=headers)) as response:
                self.assertIn("x3_ai_router_spent_usd 1.5e-05", response.read().decode())
        finally:
            if previous is None:
                os.environ.pop("X3_ROUTER_TOKEN", None)
            else:
                os.environ["X3_ROUTER_TOKEN"] = previous
            server.shutdown()
            server.server_close()

    def test_paid_price_freshness_and_free_id(self):
        paid = {"api_key_env": "TEST_KEY", "model": "paid", "input_usd_per_million": 1,
                "output_usd_per_million": 2, "pricing_checked_on": "2020-01-01"}
        self.assertEqual(router_module.pricing_error(paid), "refresh provider pricing")
        free = {"api_key_env": "TEST_KEY", "model": "nvidia/example:free", "free_model": True,
                "pricing_checked_on": router_module.dt.datetime.now(router_module.dt.timezone.utc).date().isoformat()}
        self.assertIsNone(router_module.pricing_error(free))
        free["model"] = "nvidia/example"
        self.assertIn("must use a :free ID", router_module.pricing_error(free))

    def test_task_binding_cost_and_outcome(self):
        revision = "a" * 40
        self.router.begin_task("task-1", "alice", revision, "router")
        status, _ = self.router.complete({"messages": [{"content": "format"}], "max_tokens": 10}, "alice")
        self.assertEqual(status, 200)
        evidence = {"task_id": "task-1", "revision": revision, "scope": "router",
                    "checks": [{"name": "router-tests", "exit_code": 0, "output_sha256": "b" * 64}]}
        with self.assertRaises(ValueError):
            self.router.task_outcome(evidence)  # Cannot finalize in-flight work.
        self.router.end_task_request(123)
        self.assertEqual(self.router.task_stats()[0]["cost_usd"], 0.000015)
        self.assertEqual(self.router.task_stats()[0]["elapsed_ms"], 123)
        with self.assertRaises(ValueError):
            self.router.task_outcome(dict(evidence, revision="c" * 40))
        self.assertEqual(self.router.task_outcome(evidence)["outcome"], "checks_passed")
        self.assertEqual(self.router.learning_stats()[0]["pass_rate"], 1)
        self.assertEqual(self.router.learning_stats()[0]["cost_per_passed_task_usd"], 0.000015)
        with self.assertRaises(ValueError):
            self.router.begin_task("task-1", "alice", revision, "router")

    def test_builder_cannot_submit_verification(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), router_module.handler_for(self.router))
        threading.Thread(target=server.serve_forever, daemon=True).start()
        previous = {k: os.environ.get(k) for k in ("X3_ROUTER_TOKEN", "X3_VERIFIER_TOKEN")}
        os.environ["X3_ROUTER_TOKEN"], os.environ["X3_VERIFIER_TOKEN"] = "builder", "verifier"
        try:
            request = urllib.request.Request(f"http://127.0.0.1:{server.server_port}/v1/tasks/outcome", b"{}",
                                             {"Authorization": "Bearer builder"})
            with self.assertRaises(urllib.error.HTTPError) as rejected:
                urllib.request.urlopen(request)
            self.assertEqual(rejected.exception.code, 403)
        finally:
            for k, value in previous.items():
                if value is None:
                    os.environ.pop(k, None)
                else:
                    os.environ[k] = value
            server.shutdown()
            server.server_close()

    def test_free_provider_requires_opt_in(self):
        self.config["routes"]["routine"] = ["free", "up"]
        self.config["providers"]["free"] = {"base_url": f"http://127.0.0.1:{self.upstream.server_port}/v1",
            "model": "nvidia/example:free", "free_model": True, "enabled_env": "X3_ENABLE_FREE_CLOUD_TEST",
            "api_key_env": "OPENROUTER_TEST_KEY", "pricing_checked_on": router_module.dt.datetime.now(router_module.dt.timezone.utc).date().isoformat()}
        request = {"messages": [{"role": "user", "content": "format this"}], "max_tokens": 10}
        os.environ.pop("X3_ENABLE_FREE_CLOUD_TEST", None)
        self.router.complete(request, "alice")
        self.assertEqual(Provider.requests[-1]["model"], "up")
        os.environ["X3_ENABLE_FREE_CLOUD_TEST"] = "1"
        os.environ["OPENROUTER_TEST_KEY"] = "test"
        try:
            self.router.complete(request, "alice")
            self.assertEqual(Provider.requests[-1]["model"], "nvidia/example:free")
        finally:
            os.environ.pop("X3_ENABLE_FREE_CLOUD_TEST", None)
            os.environ.pop("OPENROUTER_TEST_KEY", None)

    # ── Budget validation ────────────────────────────────────────────────

    def test_multiple_completions_are_refused(self):
        """`n` multiplies the bill; the reservation only covers one completion."""
        status, body = self.router.complete({"messages": [{"content": "format"}], "max_tokens": 10, "n": 2}, "alice")
        self.assertEqual(status, 400)
        self.assertIn("n must be 1", body["error"]["message"])
        self.assertEqual(Provider.requests, [], "a refused request must not reach a provider")

        status, _ = self.router.complete({"messages": [{"content": "format"}], "max_tokens": 10, "n": 1}, "alice")
        self.assertEqual(status, 200)

    def test_max_completion_tokens_is_bounded_and_reserved(self):
        status, body = self.router.complete({"messages": [{"content": "format"}], "max_completion_tokens": 999999}, "alice")
        self.assertEqual(status, 400)
        self.assertIn("max_completion_tokens", body["error"]["message"])

        # 32768 output tokens at $1/M plus the 1000-byte input bound is more than
        # the 0.01 daily budget. The estimate used to read only `max_tokens` and
        # fall back to 4096, so this request was served and billed afterwards.
        status, _ = self.router.complete({"messages": [{"content": "format"}], "max_completion_tokens": 32768}, "alice")
        self.assertEqual(status, 429)
        self.assertEqual(Provider.requests, [])

    def test_stream_refuses_multiple_completions(self):
        chunks = []
        status, body = self.router.stream({"messages": [{"content": "format"}], "max_tokens": 10, "n": 4},
                                          "alice", lambda: None, chunks.append)
        self.assertEqual(status, 400)
        self.assertIn("n must be 1", body["error"])
        self.assertEqual(chunks, [])

    # ── Crash recovery ───────────────────────────────────────────────────

    def test_orphaned_reservations_are_reclaimed_on_startup(self):
        day = router_module.dt.datetime.now(router_module.dt.timezone.utc).date().isoformat()
        self.router.db.execute("INSERT INTO reservations VALUES (?,?,?,?,?)",
                               ("orphan", day, "alice", 0.009, router_module.time.time() - 100_000))
        self.router.db.commit()
        self.assertGreater(self.router.snapshot()["reserved_usd"], 0)

        restarted = router_module.Router(self.config, self.tmp.name + "/usage.db")

        self.assertEqual(restarted.snapshot()["reserved_usd"], 0)
        self.assertEqual(restarted.reconciled_orphans, 1)
        self.assertIsNotNone(restarted.reserve("alice", 0.001), "the reclaimed budget must be usable again")

    def test_a_live_reservation_is_not_reclaimed(self):
        self.assertIsNotNone(self.router.reserve("alice", 0.009))
        restarted = router_module.Router(self.config, self.tmp.name + "/usage.db")
        self.assertEqual(restarted.reconciled_orphans, 0)
        self.assertGreater(restarted.snapshot()["reserved_usd"], 0)

    # ── Provider cooldowns ───────────────────────────────────────────────

    def test_budget_exhaustion_falls_through_to_a_free_provider(self):
        """Running out of money must stop the spending, not stop the work."""
        self.config["providers"]["local"] = {
            "base_url": f"http://127.0.0.1:{self.upstream.server_port}/v1",
            "model": "local", "critical_allowed": True}
        self.config["routes"]["critical"] = ["up"]
        self.config["budget_fallback"] = ["local"]
        self.config["daily_budget_usd"] = 0.000001
        self.config["agent_daily_budget_usd"] = 0.000001

        status, response = self.router.complete(
            {"messages": [{"content": "review this atomic settlement path"}], "max_tokens": 10}, "alice")

        self.assertEqual(status, 200, "the fallback provider must still answer")
        self.assertEqual(response["choices"][0]["message"]["content"], "ok")
        self.assertEqual(Provider.requests[-1]["model"], "local")

    def test_budget_exhaustion_still_refuses_paid_providers(self):
        self.config["routes"]["critical"] = ["up"]
        self.config["budget_fallback"] = []
        self.config["daily_budget_usd"] = 0.000001
        self.config["agent_daily_budget_usd"] = 0.000001

        status, body = self.router.complete(
            {"messages": [{"content": "review this atomic settlement path"}], "max_tokens": 10}, "alice")

        self.assertEqual(status, 429)
        self.assertEqual(body["error"]["type"], "budget_exceeded")
        self.assertEqual(Provider.requests, [], "an over-budget paid provider must not be called")

    def test_failing_provider_is_cooled_down_and_skipped(self):
        status, _ = self.router.complete({"messages": [{"content": "format"}], "max_tokens": 10}, "alice")
        self.assertEqual(status, 200, "the second provider still serves the request")
        self.assertGreater(self.router.provider_cooldown("down"), 0)
        health = {row["provider"]: row for row in self.router.provider_health()}
        self.assertEqual(health["down"]["failures"], 1)

        # A route holding only the cooled-down provider fails without calling it.
        self.config["routes"]["routine"] = ["down"]
        status, body = self.router.complete({"messages": [{"content": "format"}], "max_tokens": 10}, "alice")
        self.assertEqual(status, 502)
        self.assertIn("cooling down", " ".join(body["error"]["attempts"]))
        self.assertEqual(len(Provider.requests), 1, "the cooled-down endpoint must not be retried")

    def test_retry_after_is_honoured_and_success_clears_it(self):
        self.router.note_provider_failure("down", "HTTP 429", 120)
        self.assertGreater(self.router.provider_cooldown("down"), 110)
        self.router.note_provider_success("down")
        self.assertEqual(self.router.provider_cooldown("down"), 0)

    def test_cooldown_backs_off_across_consecutive_failures(self):
        self.router.note_provider_failure("down", "timeout")
        first = self.router.provider_cooldown("down")
        self.router.note_provider_failure("down", "timeout")
        second = self.router.provider_cooldown("down")
        self.assertGreater(second, first)

    # ── Client compatibility ─────────────────────────────────────────────

    def test_tool_call_requests_are_forwarded_unchanged(self):
        tools = [{"type": "function", "function": {"name": "read_file", "parameters": {"type": "object"}}}]
        status, _ = self.router.complete({"messages": [{"role": "user", "content": "read a file"}],
                                          "max_tokens": 10, "tools": tools, "tool_choice": "auto"}, "alice")
        self.assertEqual(status, 200)
        self.assertEqual(Provider.requests[0]["tools"], tools)
        self.assertEqual(Provider.requests[0]["tool_choice"], "auto")
        self.assertEqual(Provider.requests[0]["model"], "up", "the router picks the model, the client's is ignored")

    def test_models_and_unsupported_endpoints(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), router_module.handler_for(self.router))
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            url = f"http://127.0.0.1:{server.server_port}"
            with urllib.request.urlopen(url + "/v1/models/x3-auto") as response:
                self.assertEqual(json.loads(response.read())["id"], "x3-auto")
            with self.assertRaises(urllib.error.HTTPError) as unknown:
                urllib.request.urlopen(url + "/v1/models/gpt-9")
            self.assertEqual(unknown.exception.code, 404)

            # A client that reaches for the Responses API must be told plainly
            # rather than handed a 404 that looks like a wrong base URL.
            request = urllib.request.Request(url + "/v1/responses", b"{}", {"Content-Type": "application/json"})
            with self.assertRaises(urllib.error.HTTPError) as malformed:
                urllib.request.urlopen(request)
            self.assertEqual(malformed.exception.code, 400, "the Responses endpoint exists and validates input")

            embeddings = urllib.request.Request(url + "/v1/embeddings", b"{}", {"Content-Type": "application/json"})
            with self.assertRaises(urllib.error.HTTPError) as unsupported:
                urllib.request.urlopen(embeddings)
            self.assertEqual(unsupported.exception.code, 501)
            self.assertIn("Chat Completions", unsupported.exception.read().decode())

            bad = json.dumps({"model": "x3-auto", "messages": [{"role": "user", "content": "hi"}], "max_tokens": 10, "n": 3}).encode()
            with self.assertRaises(urllib.error.HTTPError) as rejected:
                urllib.request.urlopen(urllib.request.Request(url + "/v1/chat/completions", bad, {"Content-Type": "application/json"}))
            self.assertEqual(rejected.exception.code, 400)
        finally:
            server.shutdown()
            server.server_close()


    # ── Responses API (the wire protocol Codex actually speaks) ──────────

    def responses_body(self, **overrides):
        body = {"model": "x3-auto", "stream": True, "instructions": "be brief",
                "input": [{"type": "message", "role": "user",
                           "content": [{"type": "input_text", "text": "hi"}]}]}
        body.update(overrides)
        return body

    def serve(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), router_module.handler_for(self.router))
        threading.Thread(target=server.serve_forever, daemon=True).start()
        return server

    def start_native_responses_provider(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), NativeResponsesProvider)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        return server

    def configure_native_responses_provider(self, server):
        os.environ["DEEPSEEK_TEST_KEY"] = "test-key"
        self.config["providers"]["native"] = {
            "base_url": f"http://127.0.0.1:{server.server_port}",
            "model": "deepseek-flash",
            "protocol": "responses",
            "api_key_env": "DEEPSEEK_TEST_KEY",
            "input_usd_per_million": 0.3,
            "output_usd_per_million": 1.2,
            "pricing_checked_on": router_module.dt.datetime.now(router_module.dt.timezone.utc).date().isoformat(),
            "critical_allowed": True,
        }
        self.config["routes"]["routine"] = ["native", "up"]
        self.config["routes"]["critical"] = ["native", "up"]

    def test_responses_native_provider_uses_responses_endpoint(self):
        native = self.start_native_responses_provider()
        self.configure_native_responses_provider(native)
        server = self.serve()
        try:
            data = json.dumps(self.responses_body(stream=False)).encode()
            request = urllib.request.Request(
                f"http://127.0.0.1:{server.server_port}/v1/responses",
                data,
                {"Content-Type": "application/json", "X-X3-Agent": "codex"},
            )
            with urllib.request.urlopen(request) as response:
                body = json.loads(response.read())
        finally:
            os.environ.pop("DEEPSEEK_TEST_KEY", None)
            server.shutdown()
            server.server_close()
            native.shutdown()
            native.server_close()

        self.assertEqual(NativeResponsesProvider.paths, ["/responses"])
        self.assertEqual(NativeResponsesProvider.requests[0]["model"], "deepseek-flash")
        self.assertNotIn("stream_options", NativeResponsesProvider.requests[0])
        self.assertEqual(body["object"], "response")
        self.assertEqual(body["output"][0]["content"][0]["text"], "native ok")
        self.assertEqual(self.router.stats()[0]["provider"], "native")
        self.assertEqual(self.router.stats()[0]["cost_usd"], 0.000009)

    def test_responses_native_stream_is_proxied_through_terminal_event(self):
        native = self.start_native_responses_provider()
        self.configure_native_responses_provider(native)
        server = self.serve()
        try:
            data = json.dumps(self.responses_body(stream=True)).encode()
            request = urllib.request.Request(
                f"http://127.0.0.1:{server.server_port}/v1/responses",
                data,
                {"Content-Type": "application/json", "X-X3-Agent": "codex"},
            )
            with urllib.request.urlopen(request) as response:
                raw = response.read().decode()
        finally:
            os.environ.pop("DEEPSEEK_TEST_KEY", None)
            server.shutdown()
            server.server_close()
            native.shutdown()
            native.server_close()

        self.assertEqual(NativeResponsesProvider.paths, ["/responses"])
        self.assertIn("event: response.completed", raw)
        self.assertIn('"delta":"native ok"', raw)
        self.assertNotIn("[DONE]", raw)
        self.assertEqual(self.router.stats()[0]["provider"], "native")
        self.assertEqual(self.router.stats()[0]["cost_usd"], 0.000009)

    def test_responses_native_failure_falls_back_to_chat_provider(self):
        native = self.start_native_responses_provider()
        self.configure_native_responses_provider(native)
        NativeResponsesProvider.fail = True
        server = self.serve()
        try:
            data = json.dumps(self.responses_body(stream=False)).encode()
            request = urllib.request.Request(
                f"http://127.0.0.1:{server.server_port}/v1/responses",
                data,
                {"Content-Type": "application/json", "X-X3-Agent": "codex"},
            )
            with urllib.request.urlopen(request) as response:
                body = json.loads(response.read())
        finally:
            os.environ.pop("DEEPSEEK_TEST_KEY", None)
            server.shutdown()
            server.server_close()
            native.shutdown()
            native.server_close()

        self.assertEqual(NativeResponsesProvider.paths, ["/responses"])
        self.assertEqual(body["output"][0]["content"][0]["text"], "ok")
        self.assertEqual(Provider.requests[-1]["model"], "up")
        health = {row["provider"]: row for row in self.router.provider_health()}
        self.assertIn("native", health)

    def test_responses_budget_and_classifier_understand_responses_shape(self):
        self.assertEqual(
            router_module.output_bound({"max_output_tokens": 777}, self.config),
            777,
        )
        self.assertEqual(
            self.router.choose({"instructions": "review atomic finality", "input": []})[0],
            "critical",
        )

    def test_responses_native_accepts_string_input(self):
        native = self.start_native_responses_provider()
        self.configure_native_responses_provider(native)
        server = self.serve()
        try:
            data = json.dumps(self.responses_body(stream=False, input="Reply with exactly native ok")).encode()
            request = urllib.request.Request(
                f"http://127.0.0.1:{server.server_port}/v1/responses",
                data,
                {"Content-Type": "application/json", "X-X3-Agent": "codex"},
            )
            with urllib.request.urlopen(request) as response:
                body = json.loads(response.read())
        finally:
            os.environ.pop("DEEPSEEK_TEST_KEY", None)
            server.shutdown()
            server.server_close()
            native.shutdown()
            native.server_close()

        self.assertEqual(body["output"][0]["content"][0]["text"], "native ok")
        self.assertEqual(NativeResponsesProvider.paths, ["/responses"])

    def test_truncated_native_stream_gets_explicit_failed_terminal_event(self):
        native = self.start_native_responses_provider()
        self.configure_native_responses_provider(native)
        NativeResponsesProvider.truncate_stream = True
        server = self.serve()
        try:
            data = json.dumps(self.responses_body(stream=True)).encode()
            request = urllib.request.Request(
                f"http://127.0.0.1:{server.server_port}/v1/responses",
                data,
                {"Content-Type": "application/json", "X-X3-Agent": "codex"},
            )
            with urllib.request.urlopen(request) as response:
                raw = response.read().decode()
        finally:
            os.environ.pop("DEEPSEEK_TEST_KEY", None)
            server.shutdown()
            server.server_close()
            native.shutdown()
            native.server_close()

        self.assertIn("event: response.failed", raw)
        self.assertIn("upstream_stream_ended", raw)
        self.assertNotIn("response.completed", raw)

    def test_native_nonstream_failed_response_falls_back_and_marks_health(self):
        native = self.start_native_responses_provider()
        self.configure_native_responses_provider(native)
        NativeResponsesProvider.terminal_failure = True
        server = self.serve()
        try:
            data = json.dumps(self.responses_body(stream=False)).encode()
            request = urllib.request.Request(
                f"http://127.0.0.1:{server.server_port}/v1/responses",
                data,
                {"Content-Type": "application/json", "X-X3-Agent": "codex"},
            )
            with urllib.request.urlopen(request) as response:
                body = json.loads(response.read())
        finally:
            os.environ.pop("DEEPSEEK_TEST_KEY", None)
            server.shutdown()
            server.server_close()
            native.shutdown()
            native.server_close()

        self.assertEqual(body["output"][0]["content"][0]["text"], "ok")
        health = {row["provider"]: row for row in self.router.provider_health()}
        self.assertIn("native", health)
        self.assertIn("response.failed", health["native"]["last_error"])

    def test_native_stream_failed_terminal_is_not_recorded_as_success(self):
        native = self.start_native_responses_provider()
        self.configure_native_responses_provider(native)
        NativeResponsesProvider.terminal_failure = True
        server = self.serve()
        try:
            data = json.dumps(self.responses_body(stream=True)).encode()
            request = urllib.request.Request(
                f"http://127.0.0.1:{server.server_port}/v1/responses",
                data,
                {"Content-Type": "application/json", "X-X3-Agent": "codex"},
            )
            with urllib.request.urlopen(request) as response:
                raw = response.read().decode()
        finally:
            os.environ.pop("DEEPSEEK_TEST_KEY", None)
            server.shutdown()
            server.server_close()
            native.shutdown()
            native.server_close()

        self.assertIn("event: response.failed", raw)
        health = {row["provider"]: row for row in self.router.provider_health()}
        self.assertIn("native", health)
        self.assertIn("response.failed", health["native"]["last_error"])

    def test_responses_request_translation(self):
        chat = router_module.responses_request_to_chat({
            "instructions": "sys",
            "input": [
                {"type": "message", "role": "developer", "content": [{"type": "input_text", "text": "dev"}]},
                {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "go"}]},
                {"type": "function_call", "call_id": "c1", "name": "exec_command",
                 "arguments": "{\"cmd\":\"ls\"}"},
                {"type": "function_call_output", "call_id": "c1", "output": "file.txt"},
            ],
            "tools": [
                {"type": "function", "name": "exec_command", "description": "run it",
                 "parameters": {"type": "object", "properties": {}}},
                {"type": "namespace", "name": "ns", "tools": [
                    {"type": "function", "name": "inner", "parameters": {"type": "object"}}]},
                {"type": "web_search"},
            ],
            "tool_choice": "auto", "max_output_tokens": 64,
        })
        self.assertEqual([m["role"] for m in chat["messages"]],
                         ["system", "system", "user", "assistant", "tool"])
        self.assertEqual(chat["messages"][3]["tool_calls"][0]["function"]["name"], "exec_command")
        self.assertEqual(chat["messages"][4]["tool_call_id"], "c1")
        self.assertEqual([t["function"]["name"] for t in chat["tools"]], ["exec_command", "inner"],
                         "namespaced tools flatten; web_search has no chat equivalent")
        self.assertEqual(chat["tool_choice"], "auto")
        self.assertEqual(chat["max_tokens"], 64)

    def test_chat_message_maps_to_responses_output(self):
        output = router_module.chat_message_to_response_output(
            {"content": "hello", "tool_calls": [{"id": "c1", "function": {"name": "f", "arguments": "{}"}}]},
            "resp_")
        self.assertEqual([item["type"] for item in output], ["message", "function_call"])
        self.assertEqual(output[0]["content"][0]["text"], "hello")
        self.assertEqual(output[1]["call_id"], "c1")
        self.assertEqual(output[1]["name"], "f")

    def test_responses_endpoint_non_streaming(self):
        server = self.serve()
        try:
            data = json.dumps(self.responses_body(stream=False, tools=[
                {"type": "function", "name": "exec_command", "description": "run it",
                 "parameters": {"type": "object", "properties": {}}}])).encode()
            request = urllib.request.Request(f"http://127.0.0.1:{server.server_port}/v1/responses",
                                             data, {"Content-Type": "application/json", "X-X3-Agent": "codex"})
            with urllib.request.urlopen(request) as response:
                body = json.loads(response.read())
        finally:
            server.shutdown()
            server.server_close()

        self.assertEqual(body["object"], "response")
        self.assertEqual(body["status"], "completed")
        self.assertEqual(body["output"][0]["type"], "message")
        self.assertEqual(body["output"][0]["content"][0]["text"], "ok")
        self.assertIn("usage", body)
        self.assertEqual(Provider.requests[0]["messages"][0], {"role": "system", "content": "be brief"})
        self.assertEqual(Provider.requests[0]["tools"][0]["function"]["name"], "exec_command")
        self.assertEqual(self.router.stats()[0]["provider"], "up", "budget accounting still applies")

    def test_responses_endpoint_streams_the_responses_event_sequence(self):
        server = self.serve()
        try:
            data = json.dumps(self.responses_body()).encode()
            request = urllib.request.Request(f"http://127.0.0.1:{server.server_port}/v1/responses",
                                             data, {"Content-Type": "application/json", "X-X3-Agent": "codex"})
            with urllib.request.urlopen(request) as response:
                raw = response.read().decode()
        finally:
            server.shutdown()
            server.server_close()

        order = ["response.created", "response.output_item.added", "response.output_text.delta",
                 "response.output_text.done", "response.output_item.done", "response.completed"]
        positions = [raw.find('"type": "' + name + '"') for name in order]
        self.assertNotIn(-1, positions, f"missing event; got {raw[:400]}")
        self.assertEqual(positions, sorted(positions), "events must arrive in the order Codex expects")
        self.assertIn('"text": "ok"', raw)


if __name__ == "__main__":
    unittest.main()
