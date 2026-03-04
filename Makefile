# Lumen Charts — Build System
#
# Usage:
#   make all             Build everything (core + SDKs)
#   make core-libs       Build the Rust core library (release)
#   make swift-sdk       Build the Swift SDK (core + header sync)
#   make swift-demo      Build core + run the Swift demo app
#   make rust-demo       Build + run the Rust demo app
#   make wasm-sdk        Build the WASM SDK via wasm-pack
#   make webgpu-demo     Build WASM SDK + start WebGPU demo server
#   make web-canvas-demo Start the Canvas 2D demo server
#   make test            Run all tests
#   make clean           Clean all build artifacts

CORE_DIR       := core
SWIFT_SDK      := sdks/swift
SWIFT_DEMO     := examples/swift-demo
WASM_SDK       := sdks/wasm
WEBGPU_DEMO    := examples/webgpu-demo
RUST_DEMO      := examples/rust-demo
CANVAS_DEMO    := examples/web-canvas-demo
LIB_PATH       := target/release
HEADER_SRC     := $(CORE_DIR)/include/chart_core.h
HEADER_DST     := $(SWIFT_SDK)/Sources/CChartCore/chart_core.h

# ── Aggregate ────────────────────────────────────────────────

.PHONY: all
all: core-libs sdks

.PHONY: sdks
sdks: swift-sdk wasm-sdk

# ── Core ─────────────────────────────────────────────────────

.PHONY: core-libs
core-libs:
	cargo build --release -p lumen-charts-core

# ── Header sync ──────────────────────────────────────────────

.PHONY: sync-header
sync-header:
	cp $(HEADER_SRC) $(HEADER_DST)

# ── Swift ────────────────────────────────────────────────────

.PHONY: swift-sdk
swift-sdk: core-libs sync-header
	cd $(SWIFT_DEMO) && LUMEN_LIB_PATH=$(abspath $(LIB_PATH)) swift build

.PHONY: swift-demo
swift-demo: core-libs sync-header
	cd $(SWIFT_DEMO) && LUMEN_LIB_PATH=$(abspath $(LIB_PATH)) swift run ChartDemo

# ── WASM ─────────────────────────────────────────────────────

.PHONY: wasm-sdk
wasm-sdk:
	cd $(WASM_SDK) && wasm-pack build --target web

.PHONY: webgpu-demo
webgpu-demo: wasm-sdk
	cd $(WEBGPU_DEMO) && ./run.sh

.PHONY: web-canvas-demo
web-canvas-demo: wasm-sdk
	cd $(CANVAS_DEMO) && ./run.sh

# ── Rust Demo ────────────────────────────────────────────────

.PHONY: rust-demo
rust-demo:
	cd $(RUST_DEMO) && cargo run --release

# ── Test & Clean ─────────────────────────────────────────────

.PHONY: test
test:
	cargo test --workspace --exclude lumen-charts-wasm

.PHONY: clean
clean:
	cargo clean
	rm -rf $(SWIFT_DEMO)/.build
	rm -rf $(WASM_SDK)/pkg
