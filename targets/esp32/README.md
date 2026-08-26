# ESP32 target family

This target-family directory owns reusable ESP32 fabrication facts and package
contracts. `fabrication` describes the supported ESP32 family members without
owning firmware, boot, flashing, or physical proof.

Concrete firmware and its repository-development build and proof mechanics
remain under `firmware/conduit-esp32-*-signal` and consume this package through
`cargo xtask esp32-firmware ...`.
