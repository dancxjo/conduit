std-host:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-std:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-triple-local:
    cargo run -p conduit -- examples/fanout-std.form --placements examples/triple-local.placements

check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
