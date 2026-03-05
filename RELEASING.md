# Releasing and Breaking Changes

This repo is a multi-crate workspace:

1. `lumen-charts-core` (`core/`) - foundational engine
2. `lumen-charts-sdk` (`rust-sdk/`) - safe Rust SDK on top of core
3. `lumen-charts-wasm` (`sdks/wasm/`) - WASM SDK on top of core
4. `lumen-charts` (`./`) - umbrella crate re-exporting core/sdk (+ wasm on wasm32)
5. `rust-demo` (`examples/rust-demo/`) - demo only, not published

## Versioning Rules

1. Follow SemVer strictly for published crates.
2. Any breaking change in a dependency-facing API requires a major bump in each directly affected published crate.
3. Keep all inter-workspace dependency versions aligned immediately in the same PR.
4. Use `path + version` for all local workspace dependencies (already configured) so local builds and publish metadata stay consistent.

## Change Impact Matrix

1. Breaking change in `core`:
   Requires `core` major bump.
   Usually requires `sdk` major bump and `wasm` major bump.
   Usually requires `lumen-charts` major bump.
2. Breaking change only in `sdk` public API:
   Requires `sdk` major bump.
   Requires `lumen-charts` major bump (because it re-exports SDK).
3. Breaking change only in `wasm` public API:
   Requires `wasm` major bump.
   `lumen-charts` major bump only if wasm-facing umbrella API/feature contract changes.
4. Non-breaking additive changes:
   Minor bump in affected crates.
5. Internal fixes only:
   Patch bump in affected crates.

## Required Bump Order (in source)

Update manifests in this order in one PR:

1. `core/Cargo.toml`
2. `rust-sdk/Cargo.toml` and `sdks/wasm/Cargo.toml` (dependency on new core version)
3. Root `Cargo.toml` (dependency on new sdk/wasm/core versions, plus root version)
4. `examples/rust-demo/Cargo.toml` (local dependency version)

## Required Publish Order

Publish in this order:

1. `lumen-charts-core`
2. `lumen-charts-sdk`
3. `lumen-charts-wasm`
4. `lumen-charts`

Reason: each step depends on the previous crate being available on crates.io.

## Pre-Publish Checklist

1. `cargo check --workspace --exclude lumen-charts-wasm`
2. `cargo check -p lumen-charts-wasm --target wasm32-unknown-unknown`
3. `cargo test --workspace --exclude lumen-charts-wasm`
4. `cargo check -p lumen-charts --features femtovg-backend`
5. Ensure README and changelog entries match final versions.
6. Ensure dependency versions in all manifests are synchronized.

## Publish Commands

Run from repo root:

```bash
cargo publish -p lumen-charts-core
cargo publish -p lumen-charts-sdk
cargo publish -p lumen-charts-wasm --target wasm32-unknown-unknown
cargo publish -p lumen-charts
```

If you want a dry run first:

```bash
cargo publish -p lumen-charts-core --dry-run
cargo publish -p lumen-charts-sdk --dry-run
cargo publish -p lumen-charts-wasm --dry-run --target wasm32-unknown-unknown
cargo publish -p lumen-charts --dry-run
```

## Tagging Strategy

Use crate-specific tags for traceability:

1. `core-vX.Y.Z`
2. `sdk-vX.Y.Z`
3. `wasm-vX.Y.Z`
4. `lumen-charts-vX.Y.Z`

Push tags only after successful publish of all affected crates.
