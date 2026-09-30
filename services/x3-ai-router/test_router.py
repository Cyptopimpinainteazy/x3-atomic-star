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


class RouterTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        Provider.requests = []
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
            with self.assertRaises(urllib.error.HTTPError) as unsupported:
                urllib.request.urlopen(request)
            self.assertEqual(unsupported.exception.code, 501)
            self.assertIn("Chat Completions", unsupported.exception.read().decode())

            bad = json.dumps({"model": "x3-auto", "messages": [{"role": "user", "content": "hi"}], "max_tokens": 10, "n": 3}).encode()
            with self.assertRaises(urllib.error.HTTPError) as rejected:
                urllib.request.urlopen(urllib.request.Request(url + "/v1/chat/completions", bad, {"Content-Type": "application/json"}))
            self.assertEqual(rejected.exception.code, 400)
        finally:
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    unittest.main()
