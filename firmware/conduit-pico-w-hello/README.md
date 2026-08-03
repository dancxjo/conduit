# Conduit Pico W demo firmware

This package links the representative static plan into a Pico W artifact for
embedded host checks. Unlike the existing USB-only `conduit-rp2040-hil` firmware,
this crate initializes the CYW43 Wi-Fi stack and exposes:

- Wi-Fi AP mode (`conduit-pico-w` SSID),
- a bounded HTTP server on port `80`,
- DNS responder on port `53`,
- DHCP responder on ports `67/68`.

The HTTP stack includes:

- `GET /network.json`
- `GET /status.json`
- `POST /conduit` for lightweight admission signaling (`kind: ping|status|admit`).

Build and inspect the exact artifact from the Conduit workspace root:

```sh
cargo xtask embedded-gate
```

The command recomputes the expected release-firmware identity from the current
lockfile, core/executor/firmware sources, memory layout, compiler, target, and
profile.
