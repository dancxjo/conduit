std-host:
    cargo run -p conduit -- examples/signal-demo.form

demo-std:
    cargo run -p conduit -- examples/signal-demo.form

demo-triple-local:
    cargo run -p conduit -- examples/triple-signal.form

check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
