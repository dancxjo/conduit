# ESP32 target family

This target-family directory owns reusable ESP32 make facts and package
contracts. `make` describes the supported ESP32 family members without
owning firmware, boot, flashing, or physical proof.

Concrete firmware products live under `targets/esp32/firmware/` and consume
this package through `cargo xtask make esp32-firmware ...`. Firmware product,
make package, runtime execution, and physical evidence remain distinct
proof and lifecycle classes even though one target family owns their paths.
