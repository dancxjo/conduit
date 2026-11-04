# RP2040 target family

This target-family directory owns the reusable RP2040 make package. It
describes target-local realization facts without owning firmware, boot,
flashing, or physical proof.

Pico W firmware and its repository-development build and proof mechanics remain
under `targets/rp2040/firmware/pico-w-signal` and consume this package through
`cargo xtask make pico ...`.

For a Body-produced joining Seed, the generic Host preparation entrance is:

```sh
cargo xtask make host prepare-seed seed.zip --export-receipt export.json --pins pins.json --output new-private-directory
```

It verifies the selected package and current invitation, then emits the exact
`application.uf2`, private `bootstrap.uf2`, public USB admission basis and a
preparation receipt. Full Seed deployment must transfer both prepared regions
through an explicitly authorized carrier. They exclude the retained credential
journal. Preparation grants no flash authority or membership, and an
application-only update must preserve the existing bootstrap. A combined UF2
transfer is not currently qualified; see the [physical journey](../../docs/proof/canonical-lifecycle-journey.md).
