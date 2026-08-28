# Optional shell façade. Every recipe delegates to one of the two canonical
# entrances; no product or repository behavior is implemented here.
conduit *args:
    conduit {{args}}

run form *args:
    conduit run {{form}} {{args}}

form-check form *args:
    conduit check {{form}} {{args}}

inspect report *args:
    conduit inspect runtime-report {{report}} {{args}}

copy *args:
    conduit copy {{args}}

xtask *args:
    cargo xtask {{args}}

# Friendly front doors; all behavior remains owned by cargo xtask / conduit.
patchbay:
    cargo xtask demo patchbay --on native

browser:
    cargo xtask host browser

std-host:
    cargo xtask host std

demo-std:
    cargo xtask host std

demo-triple-local:
    cargo xtask demo triple

body:
    cargo xtask check workspace-test-foundation

body-thumb-check:
    cargo xtask check workspace-pico

observatory:
    cargo xtask check observatory

observatory-thumb-check:
    cargo xtask check workspace-pico

system-continuity:
    cargo xtask check workspace-test-foundation

system-continuity-thumb-check:
    cargo xtask check workspace-pico

semantic-catalog:
    cargo xtask check semantic-catalog

semantic-catalog-thumb-check:
    cargo xtask check workspace-pico

check suite="workspace" *args:
    cargo xtask check {{suite}} {{args}}

prove proof *args:
    cargo xtask prove {{proof}} {{args}}

demo demonstration *args:
    cargo xtask demo {{demonstration}} {{args}}

proofs *args:
    cargo xtask proofs {{args}}

conduitos *args:
    cargo xtask conduitos {{args}}

rpi-b-plus-image:
    cargo xtask conduitos image --arch armv6 --board rpi-b-plus-v1.2 --locked

rpi-b-plus-flash device:
    cargo xtask conduitos flash --arch armv6 --board rpi-b-plus-v1.2 --device {{device}} --confirm-device {{device}} --locked

rpi-b-plus-prove serial_device:
    cargo xtask conduitos rpi-physical-proof --board rpi-b-plus-v1.2 --serial-device {{serial_device}} --locked

rpi-zero-image:
    cargo xtask conduitos image --arch armv6 --board rpi-zero-v1 --locked

rpi-zero-flash device:
    cargo xtask conduitos flash --arch armv6 --board rpi-zero-v1 --device {{device}} --confirm-device {{device}} --locked

rpi-zero-prove serial_device:
    cargo xtask conduitos rpi-physical-proof --board rpi-zero-v1 --serial-device {{serial_device}} --locked

# One live loopback std-kernel to browser-WASM-kernel Signal proof.
prove-std-browser-s4:
    cargo xtask prove std-browser-s4

# Interactive S4 toggle demo: Enter presses drive Play starts through a real WebSocket to the browser.
toggle:
    cargo xtask demo toggle

# Conduit project homepage driven by the real distributed toggle program.
site:
    cargo xtask demo site

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

check-body-readiness:
    cargo xtask check workspace-test-foundation

check-observatory-readiness:
    cargo xtask check observatory

check-semantic-catalog-readiness:
    cargo xtask check semantic-catalog

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
