#!/usr/bin/env python3
"""Local GUI backend for Foundry + Hardhat + Ganache workflows."""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import shutil
import signal
import subprocess
import threading
import time
import uuid
from collections import deque
from dataclasses import dataclass, field
from datetime import datetime, timezone
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

IGNORED_DIR_NAMES = {
    ".git",
    ".venv",
    ".mypy_cache",
    ".pytest_cache",
    "node_modules",
    "target",
    "dist",
    "build",
    "__pycache__",
}

COMMANDS: dict[str, dict[str, list[str]]] = {
    "foundry": {
        "build": ["forge", "build"],
        "test": ["forge", "test", "-vv"],
        "clean": ["forge", "clean"],
        "node": ["anvil"],
    },
    "hardhat": {
        "compile": ["npx", "hardhat", "compile"],
        "test": ["npx", "hardhat", "test"],
        "node": ["npx", "hardhat", "node"],
    },
}

HARDHAT_CONFIG_NAMES = {
    "hardhat.config.js",
    "hardhat.config.cjs",
    "hardhat.config.mjs",
    "hardhat.config.ts",
}

GANACHE_HARDFORKS = [
    "constantinople",
    "byzantium",
    "petersburg",
    "istanbul",
    "muirGlacier",
    "berlin",
    "london",
    "arrowGlacier",
    "grayGlacier",
    "merge",
    "shanghai",
]


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def timestamp_for_file() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def command_display(command: list[str]) -> str:
    return shlex.join(command)


def command_exists(name: str) -> bool:
    return shutil.which(name) is not None


def discover_ganache_appimage() -> Path | None:
    candidate = Path.home() / "Applications" / "Ganache.AppImage"
    return candidate if candidate.exists() else None


def get_tool_environment() -> dict[str, bool]:
    return {
        "forge": command_exists("forge"),
        "anvil": command_exists("anvil"),
        "node": command_exists("node"),
        "npm": command_exists("npm"),
        "npx": command_exists("npx"),
        "hardhat": command_exists("hardhat"),
        "ganache": command_exists("ganache"),
        "ganacheAppImage": discover_ganache_appimage() is not None,
    }


def install_check(environment: dict[str, bool]) -> dict[str, Any]:
    sections = [
        {
            "id": "foundry",
            "title": "Foundry (forge + anvil)",
            "installed": environment["forge"] and environment["anvil"],
            "commands": [
                "curl -L https://foundry.paradigm.xyz | bash",
                "~/.foundry/bin/foundryup",
                "echo 'export PATH=\"$HOME/.foundry/bin:$PATH\"' >> ~/.bashrc",
            ],
            "verify": ["forge --version", "anvil --version"],
        },
        {
            "id": "node",
            "title": "Node.js + npm + npx",
            "installed": environment["node"] and environment["npm"] and environment["npx"],
            "commands": [
                "curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash -",
                "sudo apt-get install -y nodejs",
            ],
            "verify": ["node -v", "npm -v", "npx --version"],
        },
        {
            "id": "hardhat",
            "title": "Hardhat",
            "installed": environment["hardhat"] or environment["npx"],
            "commands": [
                "npm install --save-dev hardhat",
                "npx hardhat --version",
            ],
            "verify": ["npx hardhat --version"],
        },
        {
            "id": "ganache",
            "title": "Ganache CLI",
            "installed": environment["ganache"],
            "commands": [
                "npm install -g ganache",
                "ganache --version",
            ],
            "verify": ["ganache --version"],
            "note": "Ganache profile start/stop in this GUI uses Ganache CLI.",
        },
    ]
    return {"sections": sections}


@dataclass(slots=True)
class Task:
    id: str
    tool: str
    action: str
    cwd: str
    command: list[str]
    status: str = "running"
    started_at: str = field(default_factory=utc_now_iso)
    finished_at: str | None = None
    return_code: int | None = None
    logs: deque[str] = field(default_factory=lambda: deque(maxlen=3000))
    _process: subprocess.Popen[str] | None = None
    _lock: threading.Lock = field(default_factory=threading.Lock)

    def push_log(self, text: str) -> None:
        with self._lock:
            self.logs.append(text)

    def set_process(self, process: subprocess.Popen[str]) -> None:
        with self._lock:
            self._process = process

    def get_process(self) -> subprocess.Popen[str] | None:
        with self._lock:
            return self._process

    def complete(self, return_code: int, status_override: str | None = None) -> None:
        with self._lock:
            if status_override is not None:
                self.status = status_override
            elif self.status != "stopped":
                self.status = "succeeded" if return_code == 0 else "failed"
            self.return_code = return_code
            self.finished_at = utc_now_iso()

    def stop(self) -> bool:
        process = self.get_process()
        if process is None:
            return False
        if process.poll() is not None:
            return False

        try:
            process.terminate()
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=2)
        except Exception as exc:
            self.push_log(f"[stop-error] {exc}")
            return False

        self.complete(process.returncode or -1, status_override="stopped")
        self.push_log("[info] task stopped by user")
        return True

    def as_dict(self, include_logs: bool = False) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "id": self.id,
            "tool": self.tool,
            "action": self.action,
            "cwd": self.cwd,
            "command": self.command,
            "commandDisplay": command_display(self.command),
            "status": self.status,
            "startedAt": self.started_at,
            "finishedAt": self.finished_at,
            "returnCode": self.return_code,
            "isRunning": self.status == "running",
        }
        if include_logs:
            with self._lock:
                payload["logs"] = list(self.logs)
        else:
            with self._lock:
                payload["logTail"] = list(self.logs)[-40:]
        return payload


class TaskManager:
    def __init__(self) -> None:
        self._tasks: dict[str, Task] = {}
        self._lock = threading.Lock()

    def _run_task(self, task: Task) -> None:
        process = task.get_process()
        if process is None or process.stdout is None:
            task.push_log("[error] process stream missing")
            task.complete(return_code=1, status_override="failed")
            return

        for line in process.stdout:
            task.push_log(line.rstrip("\n"))

        process.stdout.close()
        return_code = process.wait()
        task.complete(return_code=return_code)

    def create(self, tool: str, action: str, cwd: Path, command: list[str]) -> Task:
        task = Task(
            id=uuid.uuid4().hex[:10],
            tool=tool,
            action=action,
            cwd=str(cwd),
            command=command,
        )
        task.push_log(f"$ {command_display(command)}")
        task.push_log(f"[cwd] {cwd}")

        env = os.environ.copy()
        env.pop("ELECTRON_RUN_AS_NODE", None)

        try:
            process = subprocess.Popen(
                command,
                cwd=str(cwd),
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                bufsize=1,
                env=env,
            )
        except Exception as exc:
            task.push_log(f"[spawn-error] {exc}")
            task.complete(return_code=1, status_override="failed")
            with self._lock:
                self._tasks[task.id] = task
            return task

        task.set_process(process)
        with self._lock:
            self._tasks[task.id] = task

        worker = threading.Thread(target=self._run_task, args=(task,), daemon=True)
        worker.start()
        return task

    def get(self, task_id: str) -> Task | None:
        with self._lock:
            return self._tasks.get(task_id)

    def list(self) -> list[dict[str, Any]]:
        with self._lock:
            tasks = list(self._tasks.values())
        tasks.sort(key=lambda task: task.started_at, reverse=True)
        return [task.as_dict(include_logs=False) for task in tasks]

    def stop(self, task_id: str) -> bool:
        task = self.get(task_id)
        if task is None:
            return False
        return task.stop()


class ProjectIndex:
    def __init__(self, workspace: Path) -> None:
        self.workspace = workspace
        self._lock = threading.Lock()
        self._cache: list[dict[str, str]] = []
        self._updated_at: float = 0.0
        self._ttl_seconds = 8.0

    def _discover(self) -> list[dict[str, str]]:
        seen: set[tuple[str, str]] = set()
        results: list[dict[str, str]] = []

        for root, dirnames, filenames in os.walk(self.workspace):
            dirnames[:] = [name for name in dirnames if name not in IGNORED_DIR_NAMES]
            path = Path(root)

            if "foundry.toml" in filenames:
                key = ("foundry", str(path))
                if key not in seen:
                    seen.add(key)
                    results.append(
                        {
                            "tool": "foundry",
                            "name": path.name,
                            "path": str(path),
                            "relativePath": str(path.relative_to(self.workspace)),
                        }
                    )

            if HARDHAT_CONFIG_NAMES.intersection(filenames):
                key = ("hardhat", str(path))
                if key not in seen:
                    seen.add(key)
                    results.append(
                        {
                            "tool": "hardhat",
                            "name": path.name,
                            "path": str(path),
                            "relativePath": str(path.relative_to(self.workspace)),
                        }
                    )

        return sorted(results, key=lambda item: (item["tool"], item["relativePath"]))

    def list(self, force_refresh: bool = False) -> list[dict[str, str]]:
        now = time.time()
        with self._lock:
            needs_refresh = force_refresh or (now - self._updated_at) > self._ttl_seconds
            if needs_refresh:
                self._cache = self._discover()
                self._updated_at = now
            return list(self._cache)


@dataclass(slots=True)
class GanacheProfile:
    workspace_name: str = "x3-local-workspace"
    rpc_host: str = "127.0.0.1"
    rpc_port: int = 7545
    chain_id: int = 1337
    default_balance: str = "1000"
    total_accounts: int = 10
    auto_generate_mnemonic: bool = True
    mnemonic: str = ""
    lock_accounts: bool = False
    gas_limit: str = ""
    gas_price: str = ""
    hardfork: str = "merge"
    output_logs_to_file: bool = False
    log_file_directory: str = "logs/ganache"
    verbose_logs: bool = False
    google_analytics: bool = False
    truffle_projects: list[str] = field(default_factory=list)

    def as_dict(self) -> dict[str, Any]:
        return {
            "workspaceName": self.workspace_name,
            "rpcHost": self.rpc_host,
            "rpcPort": self.rpc_port,
            "chainId": self.chain_id,
            "defaultBalance": self.default_balance,
            "totalAccounts": self.total_accounts,
            "autoGenerateMnemonic": self.auto_generate_mnemonic,
            "mnemonic": self.mnemonic,
            "lockAccounts": self.lock_accounts,
            "gasLimit": self.gas_limit,
            "gasPrice": self.gas_price,
            "hardfork": self.hardfork,
            "outputLogsToFile": self.output_logs_to_file,
            "logFileDirectory": self.log_file_directory,
            "verboseLogs": self.verbose_logs,
            "googleAnalytics": self.google_analytics,
            "truffleProjects": self.truffle_projects,
        }


def parse_bool(value: Any, field_name: str) -> bool:
    if isinstance(value, bool):
        return value
    raise ValueError(f"'{field_name}' must be boolean")


def parse_int(value: Any, field_name: str, min_value: int, max_value: int) -> int:
    if isinstance(value, bool):
        raise ValueError(f"'{field_name}' must be a number")
    try:
        parsed = int(value)
    except Exception as exc:
        raise ValueError(f"'{field_name}' must be a number") from exc
    if parsed < min_value or parsed > max_value:
        raise ValueError(f"'{field_name}' must be between {min_value} and {max_value}")
    return parsed


def parse_string(value: Any, field_name: str, max_len: int = 4000) -> str:
    if not isinstance(value, str):
        raise ValueError(f"'{field_name}' must be a string")
    trimmed = value.strip()
    if len(trimmed) > max_len:
        raise ValueError(f"'{field_name}' exceeds max length")
    return trimmed


def parse_profile(payload: dict[str, Any]) -> GanacheProfile:
    hardfork = parse_string(payload.get("hardfork", "merge"), "hardfork", 80) or "merge"
    if hardfork not in GANACHE_HARDFORKS:
        raise ValueError(f"'hardfork' must be one of: {', '.join(GANACHE_HARDFORKS)}")

    truffle_projects_raw = payload.get("truffleProjects", [])
    if not isinstance(truffle_projects_raw, list):
        raise ValueError("'truffleProjects' must be a list")
    truffle_projects = []
    for item in truffle_projects_raw:
        truffle_projects.append(parse_string(item, "truffleProjects[]", 1000))

    return GanacheProfile(
        workspace_name=parse_string(payload.get("workspaceName", "x3-local-workspace"), "workspaceName", 120)
        or "x3-local-workspace",
        rpc_host=parse_string(payload.get("rpcHost", "127.0.0.1"), "rpcHost", 120) or "127.0.0.1",
        rpc_port=parse_int(payload.get("rpcPort", 7545), "rpcPort", 1, 65535),
        chain_id=parse_int(payload.get("chainId", 1337), "chainId", 1, 4_294_967_295),
        default_balance=parse_string(payload.get("defaultBalance", "1000"), "defaultBalance", 80) or "1000",
        total_accounts=parse_int(payload.get("totalAccounts", 10), "totalAccounts", 1, 1000),
        auto_generate_mnemonic=parse_bool(payload.get("autoGenerateMnemonic", True), "autoGenerateMnemonic"),
        mnemonic=parse_string(payload.get("mnemonic", ""), "mnemonic", 2000),
        lock_accounts=parse_bool(payload.get("lockAccounts", False), "lockAccounts"),
        gas_limit=parse_string(payload.get("gasLimit", ""), "gasLimit", 80),
        gas_price=parse_string(payload.get("gasPrice", ""), "gasPrice", 80),
        hardfork=hardfork,
        output_logs_to_file=parse_bool(payload.get("outputLogsToFile", False), "outputLogsToFile"),
        log_file_directory=parse_string(payload.get("logFileDirectory", "logs/ganache"), "logFileDirectory", 400)
        or "logs/ganache",
        verbose_logs=parse_bool(payload.get("verboseLogs", False), "verboseLogs"),
        google_analytics=parse_bool(payload.get("googleAnalytics", False), "googleAnalytics"),
        truffle_projects=truffle_projects,
    )


class GanacheProfileStore:
    def __init__(self, workspace: Path) -> None:
        self._state_dir = workspace / "tools" / "foundry-hardhat-gui" / ".state"
        self._state_dir.mkdir(parents=True, exist_ok=True)
        self._profile_path = self._state_dir / "ganache-profile.json"
        self._lock = threading.Lock()

    def load(self) -> GanacheProfile:
        with self._lock:
            if not self._profile_path.exists():
                return GanacheProfile()
            try:
                raw = json.loads(self._profile_path.read_text(encoding="utf-8"))
            except Exception:
                return GanacheProfile()
            if not isinstance(raw, dict):
                return GanacheProfile()
            try:
                return parse_profile(raw)
            except ValueError:
                return GanacheProfile()

    def save(self, profile: GanacheProfile) -> GanacheProfile:
        with self._lock:
            self._profile_path.write_text(
                json.dumps(profile.as_dict(), indent=2),
                encoding="utf-8",
            )
        return profile


class GanacheService:
    def __init__(self, workspace: Path, tasks: TaskManager, profile_store: GanacheProfileStore) -> None:
        self.workspace = workspace
        self.tasks = tasks
        self.profile_store = profile_store
        self._task_id: str | None = None
        self._last_log_file: str | None = None
        self._lock = threading.Lock()

    def capabilities(self) -> dict[str, Any]:
        ganache_bin = shutil.which("ganache")
        appimage = discover_ganache_appimage()
        return {
            "ganacheCli": ganache_bin is not None,
            "ganacheCliPath": ganache_bin,
            "ganacheAppImage": appimage is not None,
            "ganacheAppImagePath": str(appimage) if appimage else None,
            "hardforks": GANACHE_HARDFORKS,
        }

    def _current_task(self) -> Task | None:
        if self._task_id is None:
            return None
        return self.tasks.get(self._task_id)

    def _ganache_prefix(self) -> list[str]:
        ganache_bin = shutil.which("ganache")
        if ganache_bin:
            return [ganache_bin]
        if shutil.which("npx"):
            return ["npx", "ganache"]
        raise ValueError("Ganache CLI is not installed. Install it with `npm install -g ganache`.")

    def _resolve_log_file(self, profile: GanacheProfile) -> str:
        log_dir = Path(profile.log_file_directory).expanduser()
        if not log_dir.is_absolute():
            log_dir = self.workspace / log_dir
        log_dir.mkdir(parents=True, exist_ok=True)

        safe_workspace_name = re.sub(r"[^a-zA-Z0-9_-]+", "-", profile.workspace_name).strip("-")
        if not safe_workspace_name:
            safe_workspace_name = "ganache-workspace"
        return str(log_dir / f"{safe_workspace_name}-{timestamp_for_file()}.log")

    def build_command(self, profile: GanacheProfile) -> tuple[list[str], str | None]:
        command = self._ganache_prefix()
        command.extend(
            [
                "--server.host",
                profile.rpc_host,
                "--server.port",
                str(profile.rpc_port),
                "--chain.chainId",
                str(profile.chain_id),
                "--wallet.defaultBalance",
                profile.default_balance,
                "--wallet.totalAccounts",
                str(profile.total_accounts),
                "--chain.hardfork",
                profile.hardfork,
            ]
        )

        if profile.gas_limit:
            command.extend(["--miner.blockGasLimit", profile.gas_limit])
        if profile.gas_price:
            command.extend(["--miner.defaultGasPrice", profile.gas_price])
        if profile.lock_accounts:
            command.append("--wallet.lock")
        if profile.verbose_logs:
            command.append("--logging.verbose")

        if not profile.auto_generate_mnemonic:
            if not profile.mnemonic:
                raise ValueError("Mnemonic is required when auto-generate is off.")
            command.extend(["--wallet.mnemonic", profile.mnemonic])

        log_file: str | None = None
        if profile.output_logs_to_file:
            log_file = self._resolve_log_file(profile)
            command.extend(["--logging.file", log_file])

        return command, log_file

    def status(self) -> dict[str, Any]:
        with self._lock:
            task = self._current_task()
            running = task is not None and task.status == "running"
            task_payload = task.as_dict(include_logs=False) if task else None
            return {
                "running": running,
                "task": task_payload,
                "lastLogFile": self._last_log_file,
            }

    def save_profile(self, profile: GanacheProfile) -> GanacheProfile:
        return self.profile_store.save(profile)

    def load_profile(self) -> GanacheProfile:
        return self.profile_store.load()

    def start(self, profile: GanacheProfile) -> dict[str, Any]:
        with self._lock:
            existing = self._current_task()
            if existing and existing.status == "running":
                raise ValueError("Ganache profile is already running.")
            self._task_id = None

            command, log_file = self.build_command(profile)
            task = self.tasks.create(tool="ganache", action="profile", cwd=self.workspace, command=command)
            self._task_id = task.id
            self._last_log_file = log_file
            return {
                "task": task.as_dict(include_logs=False),
                "commandDisplay": command_display(command),
                "logFile": log_file,
            }

    def stop(self) -> dict[str, Any]:
        with self._lock:
            task = self._current_task()
            if task is None:
                raise ValueError("No Ganache profile task has been started.")
            if task.status != "running":
                raise ValueError("Ganache profile is not running.")

            stopped = self.tasks.stop(task.id)
            if not stopped:
                raise ValueError("Failed to stop Ganache profile task.")
            updated = self.tasks.get(task.id)
            return {"task": updated.as_dict(include_logs=False) if updated else None}


def resolve_workspace_path(workspace: Path, candidate: str) -> Path:
    requested = Path(candidate).expanduser()
    if not requested.is_absolute():
        requested = workspace / requested
    resolved = requested.resolve()
    workspace_resolved = workspace.resolve()

    if not resolved.is_dir():
        raise ValueError(f"Directory does not exist: {resolved}")
    if workspace_resolved not in resolved.parents and resolved != workspace_resolved:
        raise ValueError("Requested directory must be inside workspace")

    return resolved


def command_for(tool: str, action: str) -> list[str]:
    tool_commands = COMMANDS.get(tool)
    if tool_commands is None:
        raise ValueError(f"Unsupported tool: {tool}")
    command = tool_commands.get(action)
    if command is None:
        raise ValueError(f"Unsupported action '{action}' for tool '{tool}'")
    return command


def parse_json(raw_body: bytes) -> dict[str, Any]:
    try:
        payload = json.loads(raw_body.decode("utf-8"))
    except json.JSONDecodeError as exc:
        raise ValueError(f"Invalid JSON: {exc.msg}") from exc
    if not isinstance(payload, dict):
        raise ValueError("Payload must be an object")
    return payload


def run_payload_from_json(raw_body: bytes) -> dict[str, str]:
    payload = parse_json(raw_body)
    tool = payload.get("tool")
    action = payload.get("action")
    cwd = payload.get("cwd")
    if not isinstance(tool, str) or not tool:
        raise ValueError("Missing 'tool'")
    if not isinstance(action, str) or not action:
        raise ValueError("Missing 'action'")
    if not isinstance(cwd, str) or not cwd:
        raise ValueError("Missing 'cwd'")
    return {"tool": tool, "action": action, "cwd": cwd}


class GuiHandler(BaseHTTPRequestHandler):
    workspace: Path
    static_dir: Path
    tasks: TaskManager
    projects: ProjectIndex
    ganache: GanacheService

    server_version = "FoundryHardhatGui/2.0"

    def log_message(self, fmt: str, *args: Any) -> None:
        del fmt, args

    def _send_json(self, payload: dict[str, Any], status: HTTPStatus = HTTPStatus.OK) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def _send_text(self, body: str, status: HTTPStatus = HTTPStatus.OK) -> None:
        encoded = body.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def _send_file(self, file_path: Path) -> None:
        if not file_path.exists() or not file_path.is_file():
            self._send_text("Not found", status=HTTPStatus.NOT_FOUND)
            return

        if file_path.suffix == ".html":
            content_type = "text/html; charset=utf-8"
        elif file_path.suffix == ".css":
            content_type = "text/css; charset=utf-8"
        elif file_path.suffix == ".js":
            content_type = "application/javascript; charset=utf-8"
        else:
            content_type = "application/octet-stream"

        body = file_path.read_bytes()
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _ganache_payload(self) -> dict[str, Any]:
        return {
            "profile": self.ganache.load_profile().as_dict(),
            "status": self.ganache.status(),
            "capabilities": self.ganache.capabilities(),
            "notes": {
                "mnemonicWarning": "This mnemonic is not secure. Never use it on a public blockchain.",
                "analyticsNote": "Google Analytics toggle is saved for workspace context only.",
            },
        }

    def do_GET(self) -> None:
        if self.path == "/" or self.path == "/index.html":
            self._send_file(self.static_dir / "index.html")
            return
        if self.path.startswith("/static/"):
            relative = self.path[len("/static/") :]
            safe_path = (self.static_dir / relative).resolve()
            if self.static_dir.resolve() not in safe_path.parents and safe_path != self.static_dir.resolve():
                self._send_text("Invalid path", status=HTTPStatus.BAD_REQUEST)
                return
            self._send_file(safe_path)
            return
        if self.path == "/api/health":
            self._send_json({"ok": True, "time": utc_now_iso()})
            return
        if self.path == "/api/environment":
            self._send_json({"environment": get_tool_environment()})
            return
        if self.path == "/api/install-check":
            self._send_json(install_check(get_tool_environment()))
            return
        if self.path.startswith("/api/projects"):
            force = "refresh=1" in self.path
            self._send_json({"projects": self.projects.list(force_refresh=force)})
            return
        if self.path == "/api/tasks":
            self._send_json({"tasks": self.tasks.list()})
            return
        if self.path.startswith("/api/tasks/"):
            task_id = self.path.split("/api/tasks/", 1)[1]
            task = self.tasks.get(task_id)
            if task is None:
                self._send_json({"error": "Task not found"}, status=HTTPStatus.NOT_FOUND)
                return
            self._send_json({"task": task.as_dict(include_logs=True)})
            return
        if self.path == "/api/ganache":
            self._send_json(self._ganache_payload())
            return

        self._send_text("Not found", status=HTTPStatus.NOT_FOUND)

    def do_POST(self) -> None:
        length = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(length) if length else b""

        if self.path == "/api/tasks":
            try:
                payload = run_payload_from_json(body)
                tool = payload["tool"]
                action = payload["action"]
                cwd = resolve_workspace_path(self.workspace, payload["cwd"])
                command = command_for(tool, action)
            except ValueError as exc:
                self._send_json({"error": str(exc)}, status=HTTPStatus.BAD_REQUEST)
                return

            task = self.tasks.create(tool=tool, action=action, cwd=cwd, command=command)
            self._send_json({"task": task.as_dict(include_logs=False)}, status=HTTPStatus.CREATED)
            return

        if self.path.startswith("/api/tasks/") and self.path.endswith("/stop"):
            task_id = self.path.removeprefix("/api/tasks/").removesuffix("/stop").strip("/")
            if not task_id:
                self._send_json({"error": "Missing task id"}, status=HTTPStatus.BAD_REQUEST)
                return
            stopped = self.tasks.stop(task_id)
            if not stopped:
                self._send_json({"error": "Task not found or not running"}, status=HTTPStatus.NOT_FOUND)
                return
            task = self.tasks.get(task_id)
            self._send_json({"task": task.as_dict(include_logs=False) if task else None})
            return

        if self.path == "/api/ganache/profile":
            try:
                payload = parse_json(body)
                profile_obj = payload.get("profile", payload)
                if not isinstance(profile_obj, dict):
                    raise ValueError("Missing profile object")
                profile = parse_profile(profile_obj)
                self.ganache.save_profile(profile)
            except ValueError as exc:
                self._send_json({"error": str(exc)}, status=HTTPStatus.BAD_REQUEST)
                return
            self._send_json(self._ganache_payload())
            return

        if self.path == "/api/ganache/start":
            try:
                payload = parse_json(body) if body else {}
                profile_obj = payload.get("profile")
                if profile_obj is not None:
                    if not isinstance(profile_obj, dict):
                        raise ValueError("profile must be an object")
                    profile = parse_profile(profile_obj)
                    self.ganache.save_profile(profile)
                else:
                    profile = self.ganache.load_profile()
                started = self.ganache.start(profile)
            except ValueError as exc:
                self._send_json({"error": str(exc)}, status=HTTPStatus.BAD_REQUEST)
                return
            self._send_json({"started": started, **self._ganache_payload()})
            return

        if self.path == "/api/ganache/stop":
            try:
                stopped = self.ganache.stop()
            except ValueError as exc:
                self._send_json({"error": str(exc)}, status=HTTPStatus.BAD_REQUEST)
                return
            self._send_json({"stopped": stopped, **self._ganache_payload()})
            return

        self._send_json({"error": "Not found"}, status=HTTPStatus.NOT_FOUND)


def make_server(workspace: Path, host: str, port: int) -> ThreadingHTTPServer:
    static_dir = Path(__file__).resolve().parent / "static"
    tasks = TaskManager()
    projects = ProjectIndex(workspace)
    ganache = GanacheService(workspace=workspace, tasks=tasks, profile_store=GanacheProfileStore(workspace))

    class Handler(GuiHandler):
        pass

    Handler.workspace = workspace
    Handler.static_dir = static_dir
    Handler.tasks = tasks
    Handler.projects = projects
    Handler.ganache = ganache
    return ThreadingHTTPServer((host, port), Handler)


def parse_args() -> argparse.Namespace:
    default_workspace = Path(__file__).resolve().parents[2]
    parser = argparse.ArgumentParser(description="Foundry + Hardhat + Ganache GUI backend")
    parser.add_argument("--host", default="127.0.0.1", help="Bind host (default: 127.0.0.1)")
    parser.add_argument("--port", type=int, default=8787, help="Bind port (default: 8787)")
    parser.add_argument(
        "--workspace",
        default=str(default_workspace),
        help=f"Workspace root to scan (default: {default_workspace})",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    workspace = Path(args.workspace).resolve()
    if not workspace.exists() or not workspace.is_dir():
        print(f"Workspace not found: {workspace}")
        return 1

    server = make_server(workspace=workspace, host=args.host, port=args.port)
    print("Foundry + Hardhat + Ganache GUI backend")
    print(f"Workspace: {workspace}")
    print(f"Open: http://{args.host}:{args.port}")

    def handle_shutdown(*_: Any) -> None:
        server.shutdown()

    signal.signal(signal.SIGINT, handle_shutdown)
    signal.signal(signal.SIGTERM, handle_shutdown)

    try:
        server.serve_forever(poll_interval=0.25)
    finally:
        server.server_close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
