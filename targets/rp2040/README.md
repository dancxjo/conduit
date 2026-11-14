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

For a package updating an already admitted physical member, replace `--pins`
with `--retained-member member.json`. This accepts the same private Make input
described below, requires retained membership of the package's Body, and emits
`usb-return.json` instead of first-admission pins. It reuses the package's exact
application and IMAGE without compiling again or replacing the member key with
the package invitation. The package and its export receipt must still verify;
preparation does not grant current membership or authorize flashing.

An already admitted member uses its retained credential and original member key,
not another invitation. The existing `make host build` private input contains
`physical_uid`, `pinned_owner`, and shared `admission` material: either `spawn`
with its `provision`, or `retained` with its `credential` and `secret`. Checked
construction binds that material to the native IMAGE and physical Host. Private
input stays separate from `.host.conduit` and the public firmware artifact.
The existing `--private-seed-input` / `--private-seed-output` options emit the
reserved bootstrap UF2 and, for retained material, public `usb-return.json`.

After deploying a matching application and bootstrap, the installed owner can
carry ordinary signed Part return over the runtime USB interface:

```sh
conduit body return-usb /dev/serial/by-id/device usb-return.json --state-dir owner-state --receipt-output new-return-receipt.json --authorize-return
```

Core commits the return; the owner saves/fsyncs its signed acknowledgement before
delivery. The MCU checks its pinned owner, retained Part and actual Boot before
acceptance. This starts no Plan or Play. Matching client/service revisions are
required: private local-control revision 2 refuses revision 1 before a return
starts. Durable MCU credential advancement across power loss and recovery after
the owner's finite pending-return window expires remain unproved.
