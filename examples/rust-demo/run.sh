#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR/../.."

echo "=== Lumen Charts — Rust Demo ==="
echo ""

# Build the core library
echo "Building core library..."
cd "$PROJECT_ROOT/core"
cargo build --release

# Build and run the demo
echo "Building and running Rust demo..."
cd "$SCRIPT_DIR"
cargo run --release
