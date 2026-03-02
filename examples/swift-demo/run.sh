#!/bin/bash
set -e

# Lumen Charts — Swift Demo Runner
# Builds the Rust core (if needed) and runs the Swift demo

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CORE_DIR="$REPO_ROOT/core"
HEADER_SRC="$CORE_DIR/include/chart_core.h"
HEADER_DST="$REPO_ROOT/sdks/swift/Sources/CChartCore/chart_core.h"

echo "==> Building Rust core (release)..."
cd "$CORE_DIR"
cargo build --release

echo "==> Syncing C header..."
cp "$HEADER_SRC" "$HEADER_DST"

echo "==> Building and running Swift demo..."
cd "$SCRIPT_DIR"
export LUMEN_LIB_PATH="$CORE_DIR/target/release"
swift run ChartDemo
