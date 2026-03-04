#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WASM_SDK="$SCRIPT_DIR/../../sdks/wasm"
PKG_DIR="$SCRIPT_DIR/pkg"
PORT="${1:-8081}"

echo "🔨 Building WASM package..."
if ! command -v wasm-pack &>/dev/null; then
    echo "❌ wasm-pack not found. Install with: cargo install wasm-pack"
    exit 1
fi

wasm-pack build "$WASM_SDK" --target web --out-dir "$PKG_DIR"

# Copy the JS wrapper API into the package directory
cp "$WASM_SDK/chart_api.js" "$PKG_DIR/"

echo ""
echo "✅ Canvas 2D Demo. Serving at http://localhost:$PORT"
echo "   Works in any browser (no WebGPU needed)."
echo "   Press Ctrl+C to stop."
echo ""

cd "$SCRIPT_DIR"
python3 -m http.server "$PORT"
