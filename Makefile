# Lumen Charts — Build System
#
# Usage:
#   make all          Build everything (core + SDKs)
#   make core-libs    Build the Rust core library (release)
#   make swift-sdk    Build the Swift SDK (core + header sync)
#   make swift-demo   Build core + run the Swift demo app
#   make wasm-sdk     Build the WASM SDK via wasm-pack
#   make wasm-demo    Build WASM SDK + start local dev server
#   make test         Run all tests
#   make clean        Clean all build artifacts

CORE_DIR     := core
SWIFT_SDK    := sdks/swift
SWIFT_DEMO   := examples/swift-demo
WASM_SDK     := sdks/wasm
WEB_DEMO     := examples/web-demo
LIB_PATH     := $(CORE_DIR)/target/release
HEADER_SRC   := $(CORE_DIR)/include/chart_core.h
HEADER_DST   := $(SWIFT_SDK)/Sources/CChartCore/chart_core.h

# ── Aggregate ────────────────────────────────────────────────

.PHONY: all
all: core-libs sdks

.PHONY: sdks
sdks: swift-sdk wasm-sdk

# ── Core ─────────────────────────────────────────────────────

.PHONY: core-libs
core-libs:
	cd $(CORE_DIR) && cargo build --release

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

.PHONY: wasm-demo
wasm-demo: wasm-sdk
	cd $(WEB_DEMO) && ./run.sh

# ── Test & Clean ─────────────────────────────────────────────

.PHONY: test
test:
	cd $(CORE_DIR) && cargo test

.PHONY: clean
clean:
	cd $(CORE_DIR) && cargo clean
	rm -rf $(SWIFT_DEMO)/.build
	rm -rf $(WASM_SDK)/pkg
