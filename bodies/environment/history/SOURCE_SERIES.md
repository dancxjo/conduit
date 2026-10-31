# Collector-observed source series

`source_series::SourceSeriesCollection::contribution(basis)` prepares an ordinary
`FaceFragment`. The Body owner composes it with the current environmental and
other contributions; it is not a new Face authority or private renderer store.
The caller owns source selection, collection, finite retention, authenticated
contribution delivery and the selected Host's complete Face/storage/work budget.

One collection admits at most four distinct exact sources, each with at most
16 observations. The input `SourceSeriesRecord { sequence, observation }` retains
the full original canonical value accepted by the existing measurement
`source_observation::ObservationContract`, including unavailable outcomes,
unsupported humidity/pressure, exact decimal coordinates, reported source
counter and explicitly untrusted reported clock. A 21-byte signed DHT record
cannot substitute for that value. No signature or device membership is invented.

The shared Face profile is:

| Subject | Fields |
| --- | --- |
| Collection | `schema = environment/source-observation-series@1`, `clock = collector-observed-boot-ms`, `collector-host` and `collector-boot` Identity, `observed-through-boot-ms` Count, `visible-series` Count |
| Series Collection, related by Contains | Stable source-session subject, friendly subject name, `source` Identity, `provenance = collector-observed-device`, `visible-samples`, `samples-outside-view`, `stale-after-ms` Counts |
| Observation Item, related by Contains | `sequence` Count and `observation` TypedValue with the exact SourceObservation contract and canonical bytes |

Observation Items follow the existing recorded-history Collection/Item pattern;
they are technical history items, not extra primary Info fieldgroups.
Contains relations do not define ordering. The reader decodes and sorts sequence
keys, rejecting duplicates, backwards collector times and observations after
the collection cutoff. Source counters do not substitute for collector sequence
numbers; missing sequence ranges and unavailable observations remain gaps.

`SourceSeriesGraph::prepare(&Face, collection_subject)` validates the profile
and builds a bounded read-only view. `PreparedSourceSeries::state_at(cutoff)`
distinguishes empty, unavailable, stale and available at the collector cutoff.
An available sample becomes stale at its declared age threshold. The outer
Face transport lease is separate: a frozen Face cannot remain live merely
because its last collector-relative state was available.

All compared observations use the exact same collector Host and Boot. The x
coordinate means **observed by that collector**, not device acquisition time,
wall time or synchronized device clocks. Original signed producer history
without a verified clock mapping remains separately scoped. Collector reboot
starts a new collection scope; a device/source-session change uses a new stable
series subject and cannot silently connect across the boundary.

Friendly names and presentation colors are not source identities. A Look/Mask
may choose a finite set of distinguishable matching line/label styles; it may
not change values, provenance, ordering, status or the clock interpretation.

This is a hosted preparation/library contract. It does not establish installed
collector execution, MCU membership, recording commitment, durable custody,
physical rendering or accepted release. The graph and live integration remain
separate qualification steps under draft PR #6633 and the existing owners.
