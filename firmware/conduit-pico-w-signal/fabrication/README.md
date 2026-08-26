# Pico W fabrication

This directory owns the Pico-specific repository-development BUILD, UF2, FLASH, and proof mechanics reached through `cargo xtask pico ...`. The firmware source, linker memory map, pinned CYW43 assets, and lightweight `fabrication-package` contribution live in the same Rust project boundary.

Generic Host construction selects exact Base implementations. The package contribution maps those selections to target-local features; this tooling performs the heavy work only after BUILD is requested.
