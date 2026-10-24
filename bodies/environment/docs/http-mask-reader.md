# Finite HTTP Mask material

`face_response` and `PreparedHttpShow` interpret one immutable environmental Face
into bounded HTML or JSON. Quantities remain upstream; only formatted Fahrenheit
and humidity coordinates, exact Face correlation and coordinate labels are
retained. Property lookup is scoped to `environment`, so other Body subjects
cannot shadow its freshness or quantities. The shared prepared environmental
view validates the same Face domain used by the seven-segment Mask; this HTTP
Mask selects no segment representation or hardware.

The CPU64 Show occupies 248 inline bytes, with no heap allocation during
preparation or rendering. Time labels follow the Face: a sampled coordinate
appears as `sampled_at_boot_ms`, an observed coordinate as `observed_at_boot_ms`,
and the other coordinate is null. `age_basis` names the reference. Without a
coordinate, both times and age are null. Observed time never becomes acquisition
time. Rendering age text cannot refresh a Face or extend manifestation custody.

`HttpShowMailbox` supplies one publication owner and separate Root withdrawal
custody. Root independently supplies the exact basis, clock and finite publication
and lifetime budget. Each accepted Show has a half-open same-Boot lease. Missing,
expired, not-yet-valid or withdrawn material is unavailable. Stale revisions and
invalid leases preserve prior material and quota. Producer drop withdraws it;
Root withdrawal fences late publication and never admits a replacement producer.
The CPU64 mailbox occupies 280 inline bytes and allocates no heap.

`cargo xtask prove environment http` covers the pure Mask and custody contracts.
Actual reader/route selection, socket work/deadlines, cancellation of outstanding
exchanges, native memory admission and browser Body membership remain separate
requirements. This change does not activate a native route or deploy firmware.
