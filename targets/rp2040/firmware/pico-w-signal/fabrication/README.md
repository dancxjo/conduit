# Pico W fabrication

This directory owns the Pico-specific repository-development BUILD, UF2, FLASH, and proof mechanics reached through `cargo xtask pico ...`. Firmware source and the linker memory map live in the parent firmware project; pinned CYW43 assets live in `targets/rp2040/firmware/assets/`. The reusable RP2040 fabrication package belongs to the target family at `targets/rp2040/fabrication` and is consumed by this firmware.

Generic Host construction selects exact Base implementations. The package contribution maps those selections to target-local features; this tooling performs the heavy work only after BUILD is requested.
