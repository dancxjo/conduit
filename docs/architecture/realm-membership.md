# Realm membership readiness

This is the first implementation slice for explicit realm membership.

It defines a local, machine-readable membership model in `conduit-realm`. The
model is intentionally smaller than discovery, durable identity, or network
admission. It gives the planner/operator a precise record of which host
instances are in one realm now and why a requested host was or was not admitted.

## What this slice proves

- A host can found a realm with a realm identity distinct from host, boot, and
  link identity.
- A second host can request admission and receive either acceptance or an
  explicit rejection reason.
- Three hosts can observe the same three-member realm view.
- Host identity, boot identity, and link identity are separate fields.
- Duplicate host, stale boot, denied admission, and departed membership are
  distinct states or rejection reasons.
- Multiple connection paths to the same host add links, not duplicate members.
- Losing one link marks that link down without ejecting the host while another
  membership record remains active.
- Membership and link evidence are serializable, bounded, and report dropped
  evidence with an explicit `EvidenceGap` marker.
- Restart behavior is explicit. A same-host/different-boot admission is rejected
  as `StaleBoot` until the operator chooses either to restore that member into
  the existing realm or to found a new realm identity.

## Current boundary

This crate is not a discovery service, a quorum protocol, a crypto admission
system, an automatic placement optimizer, or `.soul` durable recovery.

Restart behavior is intentionally local in this slice. `restore_member` updates
an existing member only when called explicitly. `Realm::found` creates a new
realm identity when the operator intentionally starts over after restart.
Neither path performs authenticated durable recovery.

## Checkpoint commands

```text
cargo test -p conduit-realm
cargo check -p conduit-realm --target thumbv6m-none-eabi
just check-realm-readiness
```
