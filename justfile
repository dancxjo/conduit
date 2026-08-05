std-host:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-std:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-triple-local:
    cargo run -p conduit -- examples/triple-signal.form --placements examples/triple-local.placements

browser-host:
    cargo test -p conduit-browser-host

browser-websocket-relay:
    cargo test -p conduit-browser-host std_host_sends_signal_to_browser_over_bounded_websocket_relay

triple-host-proof:
    cargo test -p conduit-browser-host triple_signal_form_fans_out_to_std_browser_and_pico_receipts

browser-wasm-check:
    cargo check -p conduit-browser-host --target wasm32-unknown-unknown

pico-host:
    cargo test -p conduit-pico-host

pico-udp-relay:
    cargo test -p conduit-pico-host std_host_sends_signal_to_pico_over_bounded_udp_relay

pico-thumb-check:
    cargo check -p conduit-pico-host --no-default-features --target thumbv6m-none-eabi

realm:
    cargo test -p conduit-realm

realm-thumb-check:
    cargo check -p conduit-realm --target thumbv6m-none-eabi

observatory:
    cargo test -p conduit-observatory

observatory-thumb-check:
    cargo check -p conduit-observatory --target thumbv6m-none-eabi

std-catalog:
    cargo test -p conduit-std-catalog

std-catalog-thumb-check:
    cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi

check:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

check-realm-readiness:
    cargo test -p conduit-realm
    cargo check -p conduit-realm --target thumbv6m-none-eabi

check-observatory-readiness:
    cargo test -p conduit-observatory
    cargo check -p conduit-observatory --target thumbv6m-none-eabi

check-std-catalog-readiness:
    cargo test -p conduit-std-catalog
    cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi

check-browser-readiness:
    @if cargo tree -p conduit-runtime --edges normal --prefix none | rg -q '^conduit-signal '; then echo 'conduit-runtime must not depend on conduit-signal'; exit 1; fi
    cargo check -p conduit-signal --no-default-features
    cargo check -p conduit-wire --no-default-features
    cargo test -p conduit-wire
    cargo test -p conduit-runtime --test host_contract
    cargo test -p conduit-browser-host
    cargo test -p conduit-browser-host triple_signal_form_fans_out_to_std_browser_and_pico_receipts
    cargo check -p conduit-browser-host --target wasm32-unknown-unknown
    cargo test -p conduit-pico-host
    cargo test -p conduit-pico-host std_host_sends_signal_to_pico_over_bounded_udp_relay
    cargo check -p conduit-pico-host --no-default-features --target thumbv6m-none-eabi
    @if rg -i 'playwright' -g 'Cargo.toml' -g 'package.json' -g 'package-lock.json' .; then echo 'Playwright dependency is forbidden before browser host work'; exit 1; fi
