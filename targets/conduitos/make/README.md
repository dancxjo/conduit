# ConduitOS make family

This directory owns repository-development BUILD, IMAGE, BOOT, and proof mechanics for the exact ConduitOS target family. `cargo xtask make conduitos ...` is the public repository-development entrance and routes here; generic host make does not own these target mechanics.

The lightweight package contribution is this directory's `Cargo.toml` and `src/`; repository build orchestration lives in `xtask/`. It advertises the finite x86_64, IA-32, AArch64, RISC-V64, and LoongArch64 targets without loading or running their builders during inspection.

Architecture conformance and product readiness remain separate scoreboards. A declared target does not claim BOOT or usable-host proof beyond the existing records.

The local compositor comparison enters through
`cargo xtask make conduitos compositor-performance`. It checks exact framebuffer
and coverage parity with the retained per-pixel reference, including partial
scanout failure and allocation-free composition. It then prints alternating
optimized wall-time batches for full redraw, one-pixel damage, scrolling,
hover and typing-shaped damage on a memory target. Keep raw output and source
identity when comparing runs. These measurements do not establish QEMU input
latency or physical display performance; the ordinary graphical product journey
retains its separate exact-image proof.
