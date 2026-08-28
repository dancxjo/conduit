# ESP32 target family

This target-family directory owns reusable ESP32 fabrication facts and package
contracts. `fabrication` describes the supported ESP32 family members without
owning firmware, boot, flashing, or physical proof.

Concrete firmware products live under `targets/esp32/firmware/` and consume
this package through `cargo xtask esp32-firmware ...`. Firmware product,
fabrication package, runtime execution, and physical evidence remain distinct
proof and lifecycle classes even though one target family owns their paths.
