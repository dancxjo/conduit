# Host Observatory

`conduit-observatory` is a read-only projection over neutral current-model
reports. It does not open carriers, activate or cancel Plays, edit forms, grant
authority, install providers, discover hosts, or maintain fleet membership.

## Authoritative input

One versioned `ObservatorySnapshot` contains:

- exact host advertisements plus separately reported host and capability state;
- exact directional `LinkBinding` observations and their separately reported
  operational state;
- verified plans and fragments;
- boot-scoped active and terminal Play reports;
- per-Play placement and connection lifecycle, terminal disposition, failure,
  and optional measured pressure;
- runtime-issued observations with distinct Play, presentation, and evidence
  identities;
- a finite retention capacity, retained item count, and dropped item count.

Missing facts remain unknown. In particular, the projection does not infer
reachability from membership, authority from availability, or pressure events
from planned queue limits.

`validate_snapshot` rejects unsupported schemas, invalid or duplicate plans,
duplicate hosts/boots, links whose endpoints lack exact host reports, Plays or
evidence naming unknown identities, presentation evidence without a Play, and
inconsistent retention accounting.

## Operator path

A normal actual std execution can write its authoritative snapshot:

```text
cargo run -p conduit -- examples/signal-demo.form \
  --placements examples/std-local.placements \
  --report runtime-report.json
```

Inspection is a separate read-only command:

```text
cargo run -p conduit -- observatory-report runtime-report.json
```

The inspection command only validates, projects, and renders the stored
snapshot. It does not prepare, activate, cancel, release, or otherwise control
runtime work. A tampered host/boot or other unresolved identity fails closed.

## Structured representation

The report provides complete table-shaped rows for hosts, capabilities, links,
plans, fragments, placements, connections, Plays, Play placements, Play
connections, evidence, and retention. Text rendering uses those same rows; no
graph canvas or UI state is required.

Capabilities keep kind, contract, execution profile, implementation, limits,
freshness, support, and availability separate. Plays keep plan, host, boot,
placement, connection, pressure, failure, terminal disposition, presentation,
and evidence identities separate. Pressure is `unknown` unless an authoritative
producer supplies measurements.

Host-level `EvidenceGap` counts and snapshot-level retention loss are summed for
visibility while remaining separately described in the retention explanation.

## Checkpoint commands

```text
cargo fmt --all --check
cargo test -p conduit-observatory
cargo check -p conduit-observatory --target thumbv6m-none-eabi
cargo test -p conduit
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
