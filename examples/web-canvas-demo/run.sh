#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PORT="${1:-8081}"

echo ""
echo "✅ Canvas 2D Demo — no build step required!"
echo "   Serving at http://localhost:$PORT"
echo "   Works in any browser (no WebGPU needed)."
echo "   Press Ctrl+C to stop."
echo ""

cd "$SCRIPT_DIR"
python3 -m http.server "$PORT"
