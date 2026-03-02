# Lumen Charts — Build System
# Usage:
#   make              Build the Rust core (release)
#   make swift-demo   Build core + run Swift demo
#   make wasm         Build WASM SDK
#   make test         Run all tests
#   make clean        Clean build artifacts

CORE_DIR     := core
SWIFT_SDK    := sdks/swift
SWIFT_DEMO   := examples/swift-demo
WASM_SDK     := sdks/wasm
LIB_PATH     := $(CORE_DIR)/target/release
HEADER_SRC   := $(CORE_DIR)/include/chart_core.h
HEADER_DST   := $(SWIFT_SDK)/Sources/CChartCore/chart_core.h

# Default: build the core
.PHONY: all
all: core

# Build Rust core (release)
.PHONY: core
core:
	cd $(CORE_DIR) && cargo build --release

# Sync the C header into the Swift SDK
.PHONY: sync-header
sync-header:
	cp $(HEADER_SRC) $(HEADER_DST)

# Build and run the Swift demo
.PHONY: swift-demo
swift-demo: core sync-header
	cd $(SWIFT_DEMO) && LUMEN_LIB_PATH=$(abspath $(LIB_PATH)) swift run ChartDemo

# Build the Swift SDK only (no run)
.PHONY: swift-build
swift-build: core sync-header
	cd $(SWIFT_DEMO) && LUMEN_LIB_PATH=$(abspath $(LIB_PATH)) swift build

# Build WASM SDK
.PHONY: wasm
wasm:
	cd $(WASM_SDK) && wasm-pack build --target web

# Run all tests
.PHONY: test
test:
	cd $(CORE_DIR) && cargo test

# Clean build artifacts
.PHONY: clean
clean:
	cd $(CORE_DIR) && cargo clean
	rm -rf $(SWIFT_DEMO)/.build
	rm -rf $(WASM_SDK)/pkg
