# Typed retention implementation preparation

A `no_std` implementation helper for a finite terminal input Back. It borrows one
exact immutable canonical schema and allocates its two value buffers before Play.
Core's existing borrowed validator checks representation and primitive leaves;
native-law witnesses and execution authority remain separate obligations.

A candidate becomes inspectable only through `StepBack::step_committed` after the
ordinary kernel commits input consumption. A shared finite `CommitOrder` reserves
capacity before consumption. Failed consumption, a new uncommitted step,
cancellation and Drop release the original reservation. A sequence numbers local
commits; it is not a clock, cross-Boot identity, media receipt or transport ACK.

This defines no portable Kind and does not replace `state/value`: that Kind has
authored initialization and a current/next transition boundary. It also installs
no durable store, recorder, provider or admission grant. The application supplies
its own typed contract, channel meaning, exact preparation bounds and scope.

The helper is registered in the workspace's Foundation proof shard. Repository
validation enters through `cargo xtask ci pipeline unit foundation`. The
application publication adapter retains its own channel meaning; deployment and
installed admission remain separate. Component tests drive the production StepBack interface with
test frames. Additional CPU tests use the actual fixed scheduler and bounded
input/value/sign stores to exercise consumption, commit, closure, refusal and
cancellation without allocation. These fixtures do not claim a checked or
admitted Body, planned Line, native-law witness or actual hardware. No
environmental package is a dependency.

Both the earlier Body-local checker and the prepared borrowed validator call
Core's primitive-leaf validation. Malformed leaves remain refused; this does not
introduce a stricter public Type contract. Native-law witnesses remain separate.

## Environmental adapter qualification

The application adapter retains its channel, exact linked-schema comparison and
Type-derived preparation ceiling, and delegates storage/commit/cancel to this
helper. Its channel Kind identities and local order API remain unchanged. The
old duplicate buffer/order implementations are removed. This is no durable
recording or admission claim.

CPU64 before/after preparation measurements:

| Scope | Before | After |
| --- | ---: | ---: |
| Inline application Back | 120 B | 120 B |
| Reading value buffers | 4548 B | 4548 B |
| Capture-timeout value buffers | 448 B | 448 B |
| Each empty outcome's value buffers | 42 B | 42 B |
| Preparation allocations per channel | 4 | 4 |
| Preparation reallocations | 0 | 0 |

Scoped preparation peak and requested bytes also remain identical per channel.
The contract and shared order are prepared outside this measurement. Immutable
schema residence, allocator metadata, kernel storage, firmware flash, stack and
physical Root peaks are not included. Genericity here removes duplicated code;
it does not claim a measured hardware memory reduction. Raw before/after logs
and their comparison remain in the task's external evidence area.

The prepared adapter passes all 64 environmental runtime tests with all features.
The shared package passes ten tests, including three through the actual fixed
scheduler. Both packages pass strict all-target Clippy (runtime with all
features), and the runtime adapter checks for `thumbv6m-none-eabi` without default
features. These are CPU/compile results, not firmware linking, native resource
admission, hardware memory measurements or accepted release proof.

The focused delivery refresh against the shared extraction and current `dev`
passes 59 environmental runtime tests across 19 binaries, ten helper tests,
nine workspace shard guards, workspace formatting, the evidence-retention guard,
and a Cortex-M0+ (`thumbv6m-none-eabi`) check of this shared helper. The earlier
64-test cohort above predates the device extraction; these are distinct scopes.
Neither cohort establishes live deployment or durable recording.
