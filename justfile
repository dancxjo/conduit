default:
    @just --list

fmt:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace
    cargo run -p conduit-conformance -- reference conformance/v1/manifest.json
    python3 conformance/c1/verify_canonical_v1.py

embedded:
    cargo check -p conduit-core --no-default-features --target thumbv6m-none-eabi

msrv:
    cargo +1.85.0 check --workspace --all-targets
    cargo +1.85.0 check -p conduit-core --no-default-features --target thumbv6m-none-eabi

cli-assets:
    cargo run -p conduct --bin generate-conduct-assets

cli-assets-check:
    cargo run -p conduct --bin generate-conduct-assets -- --check

sup: fmt lint test embedded

run panel="examples/hello.panel":
    cargo run -p conduct -- {{panel}}
