#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "Usage: $0 '<prompt>'"
  exit 1
fi

PROMPT="$1"

echo "Asking GPU worker: $PROMPT"
if command -v ollama >/dev/null 2>&1; then
  ollama query qwen3:8b --prompt "$PROMPT"
else
  echo "ERROR: ollama not installed or not available in PATH."
  exit 1
fi
