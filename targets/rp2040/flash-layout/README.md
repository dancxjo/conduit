# Pico flash layout

The Pico/Pico W target's 2 MiB flash layout has one source:
[`../deployment/browser/flash-layout.mjs`](../deployment/browser/flash-layout.mjs).
The browser imports its frozen data directly. The build script accepts only its
literal wrapper, validates the exact geometry, and generates allocation-free
Rust constants and a linker script. No JavaScript or JSON parser runs on the MCU.

Firmware ends before the two 4 KiB credential-journal sectors. The existing
Body bootstrap stays in the final 4 KiB sector at `0x101ff000`. Firmware linking,
bootstrap loading, and browser/native UF2 page validation consume these bounds.
Sparse browser deployment erases only sectors containing admitted image pages.
A data region omitted from an update must not become part of its erase span.

These are geometry checks, not resource grants. They establish neither exclusive
FLASH possession nor a bounded write window, installed credentials, retained
Body keys, history storage, membership, or power-loss recovery on hardware.
The platform owner must establish those separately before using the shared
credential journal. Another board geometry or retained region needs an explicit
layout change; no speculative history region is reserved here.

The crate's CPU tests run in the normal foundation test shard. Browser custody
checks run through `cargo xtask check browser`; the RP2040 browser deployment
suite additionally exercises the actual target asset and module import.
