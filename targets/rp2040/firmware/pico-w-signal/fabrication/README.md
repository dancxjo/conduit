# Pico W fabrication

This directory owns the Pico-specific repository-development BUILD, UF2, FLASH, and proof mechanics reached through `cargo xtask pico ...`. The firmware source, linker memory map, and pinned CYW43 assets remain here. The reusable RP2040 fabrication package belongs to the target family at `targets/rp2040/fabrication` and is consumed by this firmware.

Generic Host construction selects exact Base implementations. The package contribution maps those selections to target-local features; this tooling performs the heavy work only after BUILD is requested.
