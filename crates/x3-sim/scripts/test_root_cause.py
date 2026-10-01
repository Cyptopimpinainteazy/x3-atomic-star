#!/usr/bin/env python3
"""Tests for the root-cause dispatcher.

The dispatcher is the boundary between machine-produced evidence and a model
answer, so the tests pin the two things that matter: the packet is fully
represented in the prompt, and a missing or non-conforming router answer never
produces a stored "cause".
"""

import json
import sys
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import root_cause  # noqa: E402


def sample_packet(failure_id="abc123", schema="x3-failure-packet-v1"):
    return {
        "schema": schema,
        "failure_id": failure_id,
        "producer": "x3-sim",
        "commit": "deadbeef",
        "seed": 928441783,
        "scenario": "claim-refund-race",
        "config": {"sessions": 12, "steps": 200, "nodes": 5},
        "invariant": "REFUND_AFTER_CLAIM",
        "session": "sim-3",
        "detail": "phase=Complete then Refunded",
        "first_bad_step_label": "step 42",
        "first_bad_op": "refund",
        "active_faults": ["0042 partition [0]|[1, 2, 3]"],
        "state_before": {"phase": "Complete"},
        "state_after": {"phase": "Refunded"},
        "suspected_code": [
            {
                "file": "crates/cross-vm-coordinator/src/state_machine.rs",
                "symbol": "SwapCoordinator::abort",
                "reason": "must honour the terminal-phase table",
            }
        ],
        "replay_command": "cargo run -p x3-sim -- --seed 928441783 --scenario claim-refund-race",
        "minimized": {
            "verified": True,
            "replay_command": "cargo run -p x3-sim -- --seed 928441783 --scenario claim-refund-race --sessions 1 --steps 11 --nodes 3",
        },
    }


class ResponseServer(ThreadingHTTPServer):
    content = ""
    status = 200


class ResponseHandler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        ResponseServer.last_body = json.loads(self.rfile.read(length))
        payload = json.dumps(
            {
                "id": "resp_test",
                "model": "x3-auto",
                "usage": {"prompt_tokens": 10, "completion_tokens": 10},
                "choices": [{"message": {"content": ResponseServer.content}}],
            }
        ).encode()
        self.send_response(ResponseServer.status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, *_):
        pass


class RootCauseTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.packet_path = Path(self.tmp.name) / "packet.json"
        self.packet_path.write_text(json.dumps(sample_packet()))

    def tearDown(self):
        self.tmp.cleanup()

    def test_prompt_carries_every_field_an_investigator_needs(self):
        prompt = root_cause.build_prompt(sample_packet())
        for expected in (
            "REFUND_AFTER_CLAIM",
            "seed=928441783",
            "step 42",
            "partition",
            "SwapCoordinator::abort",
            "state before the bad step",
            "state after the bad step",
            "minimized (verified)",
            "--sessions 1 --steps 11 --nodes 3",
        ):
            self.assertIn(expected, prompt)

    def test_dry_run_prints_the_request_without_network(self):
        code = root_cause.main([str(self.packet_path), "--dry-run"])
        self.assertEqual(code, 0)

    def test_an_unsupported_schema_is_refused(self):
        bad = Path(self.tmp.name) / "bad.json"
        bad.write_text(json.dumps(sample_packet(schema="something-else")))
        self.assertEqual(root_cause.main([str(bad), "--dry-run"]), 2)

    def test_an_unreachable_router_stores_nothing(self):
        out = Path(self.tmp.name) / "out"
        code = root_cause.main(
            [str(self.packet_path), "--router", "http://127.0.0.1:9", "--out", str(out)]
        )
        self.assertEqual(code, 2)
        self.assertFalse(out.exists(), "no directory, no cause, when the router is down")

    def test_a_contract_answer_is_stored_with_provenance(self):
        answer = {
            "causes": [
                {
                    "symbol": "SwapCoordinator::abort",
                    "file": "crates/cross-vm-coordinator/src/state_machine.rs",
                    "confidence": 0.9,
                    "reasoning": "terminal phases must be immutable",
                }
            ],
            "first_experiment": "run the minimized seed",
            "required_regression_test": "completed_swap_cannot_be_refunded",
            "notes": "hypothesis",
        }
        ResponseServer.content = json.dumps(answer)
        server = ThreadingHTTPServer(("127.0.0.1", 0), ResponseHandler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            out = Path(self.tmp.name) / "out"
            code = root_cause.main(
                [
                    str(self.packet_path),
                    "--router",
                    f"http://127.0.0.1:{server.server_port}",
                    "--out",
                    str(out),
                ]
            )
        finally:
            server.shutdown()
            server.server_close()

        self.assertEqual(code, 0)
        stored = json.loads((out / "root-cause-abc123-refund_after_claim.json").read_text())
        self.assertEqual(stored["packet_failure_id"], "abc123")
        self.assertEqual(stored["answer"]["causes"][0]["symbol"], "SwapCoordinator::abort")
        self.assertTrue((out / "root-cause-abc123-refund_after_claim.md").exists())
        # The packet really went over the wire, not a summary of it.
        self.assertIn("REFUND_AFTER_CLAIM", ResponseServer.last_body["messages"][1]["content"])

    def test_a_nonconforming_answer_is_exit_3_and_never_stored_as_a_cause(self):
        ResponseServer.content = "I could not determine the cause."
        server = ThreadingHTTPServer(("127.0.0.1", 0), ResponseHandler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            out = Path(self.tmp.name) / "out"
            code = root_cause.main(
                [
                    str(self.packet_path),
                    "--router",
                    f"http://127.0.0.1:{server.server_port}",
                    "--out",
                    str(out),
                ]
            )
        finally:
            server.shutdown()
            server.server_close()

        self.assertEqual(code, 3)
        self.assertFalse((out / "root-cause-abc123-refund_after_claim.json").exists())

    def test_an_empty_truncated_answer_is_exit_3_with_the_reason_recorded(self):
        ResponseServer.content = ""
        server = ThreadingHTTPServer(("127.0.0.1", 0), ResponseHandler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            out = Path(self.tmp.name) / "out"
            code = root_cause.main(
                [
                    str(self.packet_path),
                    "--router",
                    f"http://127.0.0.1:{server.server_port}",
                    "--out",
                    str(out),
                ]
            )
        finally:
            server.shutdown()
            server.server_close()

        self.assertEqual(code, 3)
        raw = (out / "root-cause-abc123.invalid.txt").read_text()
        self.assertIn("finish_reason", raw)

    def test_a_cause_without_a_file_is_not_stored(self):
        ResponseServer.content = json.dumps(
            {"causes": [{"symbol": "SwapCoordinator::abort", "confidence": 0.5}]}
        )
        server = ThreadingHTTPServer(("127.0.0.1", 0), ResponseHandler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            out = Path(self.tmp.name) / "out"
            code = root_cause.main(
                [
                    str(self.packet_path),
                    "--router",
                    f"http://127.0.0.1:{server.server_port}",
                    "--out",
                    str(out),
                ]
            )
        finally:
            server.shutdown()
            server.server_close()

        self.assertEqual(code, 3, "a symbol with no file is not an actionable cause")
        self.assertFalse((out / "root-cause-abc123-refund_after_claim.json").exists())


if __name__ == "__main__":
    unittest.main()
