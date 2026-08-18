# Host Observatory

`conduit-observatory` is a read-only projection over neutral current-model
reports. It does not open lines, start or cancel Plays, edit forms, grant
authority, install bases, discover hosts, or maintain fleet membership.

## Authoritative input

One versioned `ObservatorySnapshot` contains:

- exact host advertisements plus separately reported host and capability state;
- exact boot-scoped Host Base identity, kind, state, and finite capacity,
  separately from semantic offers and resources;
- exact directional `LinkBinding` observations and their separately reported
  operational state;
- verified plans and fragments;
- boot-scoped active and terminal Play reports;
- per-Play placement and connection lifecycle, terminal disposition, failure,
  and optional measured pressure;
- current and retained historical runtime-issued observations with distinct
  Play, presentation, and Sign identities;
- optional sealed historical boot provenance tied to one reported Host/Boot,
  including adapter, image/build, normalized memory summary, artifacts,
  framebuffer basis, and proof classification;
- a finite retention capacity, retained item count, and dropped item count.

Missing facts remain unknown. In particular, the projection does not infer
reachability from membership, authority from availability, or pressure events
from planned queue limits.

`validate_snapshot` rejects unsupported schemas, invalid or duplicate plans,
duplicate hosts/boots/Bases/provenance, Bases or provenance naming unknown
Host/Boot identities, links whose endpoints lack exact host reports, Plays or
Signs naming unknown identities, presentation Signs without a Play, invalid
framebuffer provenance, and inconsistent retention accounting.

## Operator path

A normal actual std execution can write its authoritative snapshot:

```text
conduit run examples/hello.conduit \
  --report runtime-report.json
```

Inspection is a separate read-only command:

```text
conduit inspect runtime-report runtime-report.json
```

The inspection command only validates, projects, and renders the stored
snapshot. It does not prepare, start, cancel, release, or otherwise control
runtime work. A tampered host/boot or other unresolved identity fails closed.

## Structured representation

The v2 report provides complete table-shaped rows for hosts, capabilities,
Bases, Lines, plans, fragments, placements, connections, Plays, Play
placements, Play connections, current/historical Signs, sealed boot
provenance, and retention. Text rendering and Patchbay's deterministic linear
projection use those same rows; no graph canvas or UI state is required.

Capabilities keep kind, contract, execution profile, implementation, limits,
freshness, support, and availability separate. Plays keep plan, host, boot,
placement, connection, pressure, failure, terminal disposition, presentation,
and Sign identities separate. Pressure is `unknown` unless an authoritative
producer supplies measurements.

Host-level `SignGap` counts and snapshot-level retention loss are summed for
visibility while remaining separately described in the retention explanation.
Sealed boot provenance is historical input only. It is not projected as a
live offer, Base, service, availability fact, or authority source.

## Checkpoint commands

```text
cargo fmt --all --check
cargo test -p conduit-observatory
cargo check -p conduit-observatory --target thumbv6m-none-eabi
cargo test -p conduit
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
