# Host Observatory readiness

This is the first M3 slice for the read-only Host Observatory.

The implementation lives in `conduit-observatory`. It does not open sockets,
activate plans, edit forms, grant authority, install providers, or mutate host
runtime state. It projects already-authoritative data into complete structured
tables that can back a route, CLI report, or UI panel later.

## Source data

The report is built from:

- `HostAdvertisement` records;
- an optional `RealmView`;
- exact `Plan` records;
- retained `Observation` evidence.

## Tables

The report contains separate rows for:

- hosts;
- capabilities;
- realm links;
- plans;
- placements;
- connections;
- evidence;
- bounded-history retention.

These are intentionally table-shaped. A graph canvas can render the same data
later, but the complete view does not depend on graph layout or node editing.

## Current proof

The `conduit-observatory` tests feed the report builder the M1 std/browser/Pico
triple-host signal plan plus M2 realm membership data. The resulting report
keeps realm, host, boot, link, plan, placement, connection, capability, and
evidence identities in separate fields.

Capabilities expose kind, implementation, limits, freshness, support, and
availability as distinct fields instead of one combined badge.

State vocabulary keeps stale, unreachable, failed, unsupported, denied, and
unknown separate.

History is bounded by construction because it is projected from retained
machine-readable observations. Visible `EvidenceGap` observations are reported
with dropped-count retention text.

## Checkpoint commands

```text
cargo test -p conduit-observatory
cargo check -p conduit-observatory --target thumbv6m-none-eabi
just check-observatory-readiness
```
