#!/usr/bin/env bash
set -euo pipefail

# Keep PyTorch away from the failing 1070 and the display GPU.
# The current local torch build (2.10.0+cu128) successfully runs on the RTX 2060
# but does not support the GTX 1070's sm_61 architecture.
export CUDA_DEVICE_ORDER=PCI_BUS_ID
export CUDA_VISIBLE_DEVICES=GPU-baea27c9-70a5-1025-e717-01005f5da6b0

if [[ $# -eq 0 ]]; then
  echo "Usage: $0 <command> [args...]"
  echo "Example: $0 python3 train.py"
  exit 1
fi

exec "$@"
