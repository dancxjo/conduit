default:
    @just --list

fmt:
    cargo fmt --all --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace
    cargo run -p conduit-conformance -- reference conformance/v1/manifest.json

embedded:
    cargo check -p conduit-core --no-default-features --target thumbv6m-none-eabi

sup: fmt lint test embedded

run panel="examples/hello.panel":
    cargo run -p conduct -- {{panel}}
