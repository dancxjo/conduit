# Reluctant replacement through the ordinary Std Host

**Partial #5568 implementation. The issue remains open.** The bounded planner
gate now connects to actual installed execution through
`StdHost::run_reluctant_replacement_to`. This finishes the previously missing
recommendation-to-execution bridge. Measured cost production and the complete
independent acceptance record remain open.

## Exact execution boundary

This first profile admits one local pure expression Value Gear, one input and
one output, with unchanged external Fore contracts. It supports the existing
generic Host Call expression, scalar-tape and direct-local expression Backs.
It admits no new Back or execution kernel. Stateful, resource-bearing, Flow,
remote, multi-Gear and physically grouped replacements explicitly refuse this
profile; the general planner gate does not imply those execution capabilities.

The caller owns the old immutable Plan/report and the persistent cost gate.
The bridge requires a completed old Play, matching Plan/Play/Host/Boot and
execution-map identities, complete consecutive kernel Signs, one logical
completion and one acknowledged delivery. It scans at most 256 prior
observations and 256 kernel Signs. It binds the exact new request's typed port,
kind and bytes to every cost demand before consuming the gate. There is one
submitted Value, at most 65,536 input bytes, and two matching local Value Fores,
each with one item and at most 65,536 bytes. Ordinary kernel admission retains
its own tighter applicable limits.

A changed current advertisement refuses the retained snapshot. If the cost gate
keeps the current Plan, neither the kernel nor output adapter is called. A
recommendation enters the ordinary reservation/factory/Host Call/Fore execution
path, which still performs current admission. Old input/output is never replayed;
this is a separately submitted new request. The result owns its decision and
new execution report. Refusal/failure from the ordinary path retains the
recommendation, and the gate stays latched. An unknown external acknowledgement
therefore cannot silently trigger another attempt. No report grants authority,
restores a live Play or changes Body truth. The caller retains historical custody.

## Current proof

Seven installed-execution tests exercise:

- a distinct candidate Plan and new Play, correct output and unchanged old report;
- a noisy model that keeps the current Plan without calling the adapter;
- substituted request bytes and an unavailable old terminal outcome, without consuming the gate;
- current offer loss refusing a historical snapshot before effects;
- an adapter retaining output and returning unknown acknowledgement, followed by no automatic retry;
- foreign, duplicate, oversized, incomplete or inconsistent prior terminal/Sign evidence;
- the genuine direct-local Back through the same kernel boundary, with unchanged Value storage capacity.

The tests use checked `baseline-32` Source, genuine installed alternatives and
real kernel/Fore execution. Their clock and cost numbers are explicitly declared
conformance fixtures, not physical measurements or speedup evidence.

Implementation source `9faabe6fca5fc1e15644db73de4281390b56fc7a` passed the
supported `cargo xtask prove resource-frame` suite: **46 actual tests**, no
ignored tests. Strict scoped Clippy and focused formatting passed on the clean
unchanged source. After integrating accepted development, source
`5a45da40c20a0c6fa3aa7a42bd26af3f640d7cf0` passed all **18 relevant execution,
policy and replacement-cost tests**, the same strict Clippy and formatting.
The two snapshots are retained separately; 46 tests are not attributed to the
later source. [Commands, counts and hashed complete logs](evidence/reluctant-execution-bridge-20261020/validation.json)
record both boundaries.

## Exploratory measurements and remaining work

An internal uncommitted debug probe ran five alternating generic/scalar-tape
pairs and five generic/direct-local pairs for the same one-Gear request. Scalar
tape usually cost more once per-call preparation was included; direct-local
showed a possible alternative worth measuring. These observations changed the
next experiment rather than justifying a predetermined profitable result.
[Both raw diagnostics and the later producer snapshot](evidence/reluctant-execution-bridge-20261020/excluded-diagnostics.json)
are explicitly excluded: debug build, dirty source and no admitted replacement
costs/policy lifecycle. Their HEAD field is not a verified code pin. No release
speedup, threshold, confidence interval or general profitability follows.

Finish a clean source-pinned release family using real completed observations
and all five actual preparation/transfer/residency/migration/continuity costs,
including explicit non-applicability where this pure profile has no external
residence or mutable state. Preserve the entire producer/cost/decision/Plan/Play/
Sign/output chain. Exercise real bounded noisy-load observations, fresh ordinary
replacement admission and retained old outcomes. Independently verify the net
arithmetic and causal records, with falsification controls. If the observations
do not support persistent net benefit, retain that result rather than forcing
replacement. Foreground Body/UI replanning and automatic stable publication are
not established by this hosted execution bridge. #5568 and #6078 stay open.
