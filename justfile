hello:
    cargo run -p conduit -- examples/hello.panel

check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
