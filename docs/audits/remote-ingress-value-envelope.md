# Remote ingress respects the semantic value envelope

Development fix for #5231 atomic pressure/ownership preservation, 17 October 2026.

Local payload publication enforced `CordSpec.maximum_value_bytes`, but single
and fan-out remote ingress checked only available queue bytes. An external
payload could therefore exceed the admitted single-value envelope while fitting
the larger queue budget. For example, a three-byte input was accepted by a cord
with a two-byte semantic limit and four-byte queue capacity.

The shared remote payload admission helper now rejects that input with the
existing `SemanticValueBoundExceeded` error before checking transient pressure,
allocating/storing values, coalescing, advancing sequences, waking nodes or
publishing Signs. A payload exceeding an immutable semantic limit is a permanent
refusal, including when the queue is already full; it is not retryable `Full`.
Single and fan-out entry points use the same check. Fan-out still validates every
branch before publication. The admission/available-byte helpers are extracted
into `scheduler/payload_admission.rs`; their scheduler-internal visibility is
retained, and the oversized root shrinks. No scheduler storage is added.

Eight cases cover single vs fan-out, default vs coalescing pressure, and empty
vs occupied queues. The second branch has the narrower envelope, so fan-out
must not publish to the preceding broader branch. Refusal preserves exact queue
heads/usages/sequences, value occupancy/bytes, readiness and Sign entries.
Within-limit input still admits at the same sequence on an empty queue; global
cancellation releases all values. Temporarily disabling the new check reproduces
`Accepted` where `SemanticValueBoundExceeded` is expected.

Both complete local kernel suites pass with alloc under packed and linear ready
selectors, along with strict all-target kernel Clippy, formatting and diff
checks. This is native kernel evidence; no browser/physical/stable acceptance or
parent completion is claimed. No CI dispatched or awaited.

## Sealed typed input through whole-Plan lowering

`architecture/plan-lowering/tests/remote_value_envelope.rs` now carries a checked
two-byte value/text contract on an ordinary sealed input Fore with an independent
eight-byte queue budget. The Plan is sealed and verified, then lowered through
the public whole-Plan-aware entrance. The test verifies there are no state,
Host Call or activation obligations omitted by the native fixture, and installs
the returned node/cord tables directly into FixedScheduler.

For PreserveOrder and CoalesceLatest, Fore validation rejects a three-byte value
as Oversize and accepts the exact two-byte value. Kernel ingress independently
refuses the oversized bytes before and after the queue is occupied, preserving
queue/value occupancy and Sign count. Admission at the same sequence then succeeds
at the limit; a second within-limit input returns Full for PreserveOrder or
replaces the current item for CoalesceLatest. Occupancy stays one item/two bytes,
and cancellation releases it.

Both complete plan-lowering suites pass locally with normal and linear kernel
selectors, alongside strict all-target plan-lowering Clippy and formatting/diff
checks. Fixture value/Sign storage is explicitly fixed; this does not claim
checked authored-source execution, real Host installation, complete hosted
resource admission, live transport, browser or stable acceptance. No production
code changes, CI dispatch or CI wait are added by this proof slice.
