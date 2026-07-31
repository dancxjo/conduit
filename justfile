default:
    @just --list

fmt:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace
    cargo run -p conduit-conformance -- reference conformance/current/manifest.json
    python3 conformance/c1/verify_canonical_v1.py

embedded:
    cargo check -p conduit-core --no-default-features --target thumbv6m-none-eabi
    cargo check -p conduit-embedded --target thumbv6m-none-eabi
    python3 tools/embedded_gate.py

msrv:
    cargo +1.85.0 check --workspace --all-targets
    cargo +1.85.0 check -p conduit-core --no-default-features --target thumbv6m-none-eabi
    cargo +1.85.0 check -p conduit-embedded --target thumbv6m-none-eabi

cli-assets:
    cargo run -p conduct --bin generate-conduct-assets

cli-assets-check:
    cargo run -p conduct --bin generate-conduct-assets -- --check

perf:
    python3 tools/performance_gate.py

sup: fmt lint test embedded perf

run panel="examples/hello.panel":
    cargo run -p conduct -- {{panel}}
