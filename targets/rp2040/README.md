# RP2040 target family

This target-family directory owns the reusable RP2040 make package. It
describes target-local realization facts without owning firmware, boot,
flashing, or physical proof.

Pico W firmware and its repository-development build and proof mechanics remain
under `targets/rp2040/firmware/pico-w-signal` and consume this package through
`cargo xtask make pico ...`.

An exported Body-produced joining Seed can be prepared for the generic Host with
`cargo xtask make host prepare-seed seed.zip --export-receipt export.json
--pins pins.json --output new-private-directory` (one command). This verifies
the selected package and invitation before emitting the application, private
bootstrap, public USB admission basis, and `installation.uf2`. The latter is one
coherent mass-storage transfer containing the exact application and bootstrap;
it excludes the retained network-credential journal. Treat it as private. Seed
preparation performs no flash write, membership admission, or Play. Deployment
requires explicit authority to replace the application and bootstrap; an
application-only update must continue to preserve the bootstrap.
