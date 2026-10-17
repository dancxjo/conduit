# Prepared std Host Call identity lookup

Development slice of #5395, 17 October 2026.

The installed std runner previously searched every lowered Host Call for each
request and cancellation, then searched the retained request vector to record
a first observation. Preparation now builds a fixed 128-entry index for the
existing 32-node, four-calls-per-node profile. Play borrows the exact original
LoweredHostCall through the index; no contract, authority or effect is rebuilt.
Both dimensions are checked before flattening, so an out-of-range call cannot
alias the next node. Admission and missing-identity refusals remain explicit.

First-only recording retains the first RequestId and input in original order.
Math and Face construction retain every completion exactly as before; their
append path also updates first-observation state. The original vector is handed
to post-Play report translation unchanged. The fixed arrays use 256 bytes of
indices and 128 bytes of flags; on this native host the wrapper is 424 bytes,
400 more than the former Vec alone. Request capacity is reserved before Play
for at least all unique lowered calls. Existing append-all vector behavior is
retained; this does not establish whole-Host allocation-free execution.

The differential test permutes all 128 identities and checks 4,096 mixed
first-only and append-all records against the original linear oracles, including
pointer identity, ordering, exact requests, unchanged capacity and zero tracked
allocations. Negative tests cover empty/missing keys, each dimension outside
the profile, duplicate first-match behavior and oversized preparation.
Focused native tests also exercise scalar tape, ordinary math, layout and Face
composition, canonical allocation contracts and authored cancellation.

Before capture: clean Source 2313ac8a4d6be87474c186d0788ba09676bba2b5,
18 release conformance tests, 89 artifact hashes, 190 rows and 123 repeated
distributions independently verified. The intervening commits through the
starting dev head 130fe4a34 change only browser JavaScript/tests and its audit;
the native implementation is identical. After measurements and matched output
custody are retained separately below. Uncontrolled one-process before/after
observations cannot establish a calibrated speedup or regression gate.

No PR or CI requested, per operator instruction. Parent #5395 remains open
until its full measurement matrix and closure evidence are present.
