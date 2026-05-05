#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "Starting Ollama GPU workers for X3 swarm..."
mkdir -p "$ROOT_DIR/logs/swarm"

if ! command -v ollama >/dev/null 2>&1; then
  echo "ERROR: ollama executable not found. Install Ollama or update PATH."
  exit 1
fi

echo "Launching Ollama worker processes..."

echo "- GPU 0 / RTX: PlannerAgent + CodeAgent" > "$ROOT_DIR/logs/swarm/ollama_worker_roles.log"
echo "- GPU 1 / GTX 1070: TestBuilderAgent" >> "$ROOT_DIR/logs/swarm/ollama_worker_roles.log"
echo "- GPU 2 / GTX 1070: AuditorAgent + BreakerAgent" >> "$ROOT_DIR/logs/swarm/ollama_worker_roles.log"
echo "- GPU 3 / GTX 1070: MarketingAgent + GrantAgent" >> "$ROOT_DIR/logs/swarm/ollama_worker_roles.log"

nohup ollama serve --model qwen3:8b > "$ROOT_DIR/logs/swarm/ollama_worker.log" 2>&1 &
  echo "$!" > "$ROOT_DIR/logs/swarm/ollama_worker.pid"
  echo "Ollama GPU worker started with PID $(cat "$ROOT_DIR/logs/swarm/ollama_worker.pid")"
  echo "Role mapping written to $ROOT_DIR/logs/swarm/ollama_worker_roles.log"
