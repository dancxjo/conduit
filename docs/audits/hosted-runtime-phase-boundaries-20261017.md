# Hosted runtime preparation and Play wall boundaries

Local #5395 implementation slice, 17 October 2026. The ordinary std Host already
exposes optional `PreparationStarted`, `PlayStarted`, and `PlayFinished` markers.
The performance harness now observes those existing boundaries for each ordinary,
local and fused 1/8/32-Gear pipeline, using three fixed timestamp slots.

Each successful call retains marker offsets, observed call wall time and four
segments: before installed preparation, Back/kernel preparation, Play, and
after Play. Segments partition the observed call exactly. The outer existing
full-call measurement includes observation overhead. Markers are native wall
observations, not authored semantic time or process CPU samples. Preparation
includes all installed setup between its two boundaries; Play includes Host
coordination and required evidence; post-Play includes report translation and
other work until return. This does not isolate arithmetic, copy, individual
Sign or teardown cost. Existing cold parse/check/expand/selection/seal rows and
separate Host/harness teardown measurements are retained.

The observer rejects missing, duplicated, reordered and extra markers. It adds
no allocation or new marker to production execution; the optional observer
adds five wall-clock samples per measured call. Repeated distributions retain
the five durations as separate metrics. The capture verifier derives the
partition from raw marker offsets, enforces nonnegative monotonic bounds within
the outer call and raw/report/distribution agreement. Legacy captures without
phase observations remain distinguishable and readable; mixed partially
observed pipeline captures refuse.

Use the supported `cargo +stable xtask prove runtime-performance` entrance with
`--hosted-pipeline true` to retain these observations from clean committed
source. Performance remains report-only. Full streaming, DSP, browser, instruction
and copy attribution and stable parent acceptance remain open.
