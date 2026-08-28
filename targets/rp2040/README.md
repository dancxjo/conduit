# RP2040 target family

This target-family directory owns the reusable RP2040 fabrication package. It
describes target-local realization facts without owning firmware, boot,
flashing, or physical proof.

Pico W firmware and its repository-development build and proof mechanics remain
under `targets/rp2040/firmware/pico-w-signal` and consume this package through
`cargo xtask pico ...`.
