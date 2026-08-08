std-host:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-std:
    cargo run -p conduit -- examples/signal-demo.form --placements examples/std-local.placements

demo-triple-local:
    cargo run -p conduit -- examples/triple-signal.form --placements examples/triple-local.placements

browser-sim:
    cargo test -p conduit-browser-sim

browser-frame-fixture:
    cargo test -p conduit-browser-sim std_host_sends_signal_to_browser_through_bounded_frame_fixture

triple-sim-proof:
    cargo test -p conduit-browser-sim triple_signal_form_fans_out_to_std_and_simulated_receipts

browser-sim-wasm-check:
    cargo check -p conduit-browser-sim --target wasm32-unknown-unknown

pico-sim:
    cargo test -p conduit-pico-sim

pico-datagram-fixture:
    cargo test -p conduit-pico-sim std_host_sends_signal_to_pico_through_bounded_datagram_fixture

pico-sim-thumb-check:
    cargo check -p conduit-pico-sim --no-default-features --target thumbv6m-none-eabi

realm:
    cargo test -p conduit-realm

realm-thumb-check:
    cargo check -p conduit-realm --target thumbv6m-none-eabi

observatory:
    cargo test -p conduit-observatory

observatory-thumb-check:
    cargo check -p conduit-observatory --target thumbv6m-none-eabi

system-continuity:
    cargo test -p conduit-system-continuity

system-continuity-thumb-check:
    cargo check -p conduit-system-continuity --target thumbv6m-none-eabi

std-catalog:
    cargo test -p conduit-std-catalog

std-catalog-thumb-check:
    cargo check -p conduit-std-catalog --no-default-features --target thumbv6m-none-eabi

check:
    cargo xtask check workspace

# One live loopback std-kernel to browser-WASM-kernel Signal proof.
prove-std-browser-s4:
    cargo xtask prove std-browser-s4

# Interactive S4 toggle demo: Enter presses drive activations through a real WebSocket to the browser.
toggle:
    cargo xtask demo toggle

# One live loopback std-kernel to browser-WASM-kernel toggle proof.
prove-std-browser-toggle:
    cargo xtask prove std-browser-toggle

# Live hardware std-kernel to Pico W USB-CDC signal proof.
prove-std-pico-usb *args:
    cargo xtask prove std-pico-usb {{args}}

check-kernel-s1:
    cargo xtask check kernel-takeover

check-kernel-takeover:
    cargo xtask check kernel-takeover

check-planning-s2:
    cargo xtask check planning-s2

check-form-s3:
    cargo xtask check form-s3

check-browser-s4:
    cargo xtask check browser-host

check-realm-readiness:
    cargo xtask check realm

check-observatory-readiness:
    cargo xtask check observatory

check-std-catalog-readiness:
    cargo xtask check std-catalog

check-sim-readiness:
    cargo xtask check sim

# Inspect repository and platform prerequisites.
doctor target="all" *args:
    cargo xtask doctor {{target}} {{args}}

# Pico W local LED proof — full workflow (doctor -> build -> flash -> verify).
pico *args:
    cargo xtask pico {{args}}

pico-local *args:
    cargo xtask pico-local {{args}}

pico-doctor:
    cargo xtask pico doctor

pico-build *args:
    cargo xtask pico build {{args}}

pico-flash *args:
    cargo xtask pico flash {{args}}

pico-verify *args:
    cargo xtask pico verify {{args}}

pico-build-remote *args:
    cargo xtask pico build --usb-remote {{args}}

pico-flash-remote *args:
    cargo xtask pico flash --usb-remote {{args}}

pico-local-run:
    cargo xtask pico local
