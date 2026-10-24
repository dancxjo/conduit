# Retained publication Face

`RetainedPublicationFace` owns finite prepared semantic state for one local
publication stream. Its caller supplies the actual producing Source/checked/
expanded Plot, Plan and Play basis, optional coherent Body/Wake basis, maximum
publication count, monotonic commit clock and freshness policy. Those IDs remain
provenance, not membership, permission or proof of an installed Body.

Each committed snapshot must have the next exact sequence and match the selected
channel's checked structured contract. Replays, gaps, malformed values, wrong
channel types, reversed commit time and exhausted publication bounds refuse
without changing the previous Face or consumed source sequence. Validation and
Face storage are allocated during preparation; publication, freshness expiry and
resealing allocate nothing during Play.

The existing prepared Face preserves first-class Temperature and relative
humidity. On capture failure, previously measured quantities remain explicitly
last-known facts with unavailable sensor status and their own age. Expiry changes
only the sealed Face revision and freshness; it acquires no measurement and
advances no source sequence. Masks still own units, formatting and page timing.

This is producer-side semantic state using the ordinary Face grammar, not a
replacement for the aggregate parent Body Face. Body composition still owns the
parent's lifecycle and other contributions. Preparing this owner neither samples
hardware, installs a Mask route, opens a Line, nor admits a recording store.
It has not yet been installed in the actual MCU worker or admitted there with
its complete heap/control/future bounds.

`cargo xtask prove environment face-publication` exercises checked Source
observations, retained quantities, failure/recovery, freshness and exact refusal
atomicity. The retained-owner suite measures zero recurring allocations and
reallocations. CPU fixture provenance is explicitly distinct from live admission.
