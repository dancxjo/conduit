# ConduitOS fabrication family

This directory owns repository-development BUILD, IMAGE, BOOT, and proof mechanics for the exact ConduitOS target family. `cargo xtask conduitos ...` is the public repository-development entrance and routes here; generic Host fabrication does not own these target mechanics.

The lightweight package contribution is this directory's `Cargo.toml` and `src/`; repository build orchestration lives in `xtask/`. It advertises the finite x86_64, IA-32, AArch64, RISC-V64, and LoongArch64 targets without loading or running their builders during inspection.

Architecture conformance and product readiness remain separate scoreboards. A declared target does not claim BOOT or usable-Host proof beyond the existing records.
