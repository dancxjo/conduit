std-host:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-std:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-triple-local:
    cargo run -p conduit -- examples/triple-signal.form --placements examples/triple-local.placements

browser-host:
    cargo test -p conduit-browser-host

browser-wasm-check:
    cargo check -p conduit-browser-host --target wasm32-unknown-unknown

check:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

check-browser-readiness:
    @if cargo tree -p conduit-runtime --edges normal --prefix none | rg -q '^conduit-signal '; then echo 'conduit-runtime must not depend on conduit-signal'; exit 1; fi
    cargo check -p conduit-signal --no-default-features
    cargo check -p conduit-wire --no-default-features
    cargo test -p conduit-wire
    cargo test -p conduit-runtime --test host_contract
    cargo test -p conduit-browser-host
    cargo check -p conduit-browser-host --target wasm32-unknown-unknown
    @if rg -i 'playwright' -g 'Cargo.toml' -g 'package.json' -g 'package-lock.json' .; then echo 'Playwright dependency is forbidden before browser host work'; exit 1; fi
