"""Run the router scope on a clean commit and submit evidence as a verifier."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import urllib.request


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--task-id", required=True)
    parser.add_argument("--router-url", default="http://127.0.0.1:11435")
    args = parser.parse_args()
    token = os.environ["X3_VERIFIER_TOKEN"]
    repo = args.repo.resolve()
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=repo):
        raise SystemExit("Verifier requires a clean committed checkout")
    try:
        result = subprocess.run([sys.executable, "-m", "unittest", "discover", "-s", "services/x3-ai-router", "-p", "test_*.py", "-v"],
                                cwd=repo, capture_output=True, timeout=300)
        output, code = result.stdout + result.stderr, result.returncode
        # unittest discovery with zero tests is not evidence of success.
        if b"Ran 0 tests" in output:
            code = 1
    except subprocess.TimeoutExpired as exc:
        output, code = (exc.stdout or b"") + (exc.stderr or b""), 124
    if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip() != revision or subprocess.check_output(["git", "status", "--porcelain"], cwd=repo):
        raise SystemExit("Checkout changed during verification; evidence rejected")
    payload = {"task_id": args.task_id, "revision": revision, "scope": "router", "checks": [
        {"name": "router-tests", "exit_code": code, "output_sha256": hashlib.sha256(output).hexdigest()}]}
    request = urllib.request.Request(args.router_url.rstrip("/") + "/v1/tasks/outcome", json.dumps(payload).encode(),
                                     {"Content-Type": "application/json", "Authorization": "Bearer " + token})
    with urllib.request.urlopen(request, timeout=30) as response:
        print(response.read().decode())
    raise SystemExit(0 if code == 0 else 1)


if __name__ == "__main__":
    main()
