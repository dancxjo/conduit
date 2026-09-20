# host Observatory

`conduit-observatory` is a read-only projection over neutral current-model
reports. It does not open lines, start or cancel plays, edit forms, grant
authority, install bases, discover hosts, or maintain fleet membership.

## Authoritative input

One versioned `ObservatorySnapshot` contains:

- exact host advertisements plus separately reported host and capability state;
- exact boot-scoped host base identity, kind, state, and finite capacity,
  separately from semantic offers and resources;
- exact directional `LinkBinding` observations and their separately reported
  operational state;
- verified plans and fragments;
- boot-scoped active and terminal play reports;
- per-play placement and connection lifecycle, terminal disposition, failure,
  and optional measured pressure;
- current and retained historical runtime-issued observations with distinct
  play, presentation, and sign identities;
- optional sealed historical boot provenance tied to one reported host/boot,
  including adapter, image/build, normalized memory summary, artifacts,
  framebuffer basis, and proof classification;
- a finite retention capacity, retained item count, and dropped item count.

Missing facts remain unknown. In particular, the projection does not infer
reachability from membership, authority from availability, or pressure events
from planned queue limits.

`validate_snapshot` rejects unsupported schemas, invalid or duplicate plans,
duplicate hosts/boots/bases/provenance, bases or provenance naming unknown
host/boot identities, links whose endpoints lack exact host reports, plays or
signs naming unknown identities, presentation signs without a play, invalid
framebuffer provenance, and inconsistent retention accounting.

## Operator path

A normal actual std execution can write its authoritative snapshot:

```text
conduit run forms/hello/main.conduit \
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
bases, lines, plans, fragments, placements, connections, plays, play
placements, play connections, current/historical signs, sealed boot
provenance, and retention. Text rendering and Patchbay's deterministic linear
projection use those same rows; no graph canvas or UI state is required.

Capabilities keep kind, contract, execution profile, implementation, limits,
freshness, support, and availability separate. plays keep plan, host, boot,
placement, connection, pressure, failure, terminal disposition, presentation,
and sign identities separate. Pressure is `unknown` unless an authoritative
producer supplies measurements.

host-level `SignGap` counts and snapshot-level retention loss are summed for
visibility while remaining separately described in the retention explanation.
Sealed boot provenance is historical input only. It is not projected as a
live offer, base, service, availability fact, or authority source.

## Working on this boundary

The implementation and focused tests live in
[`architecture/observatory`](../../architecture/observatory/). Use the
[contribution guide](../../CONTRIBUTING.md) for repository checks. A saved
snapshot remains historical input; inspecting it never makes its boot or
capabilities current.
