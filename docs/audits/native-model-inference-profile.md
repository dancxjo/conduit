# Native scalar named-inference Back (#5694)

The optional std/Linux x86_64 glibc profile
`std/ort-cpu-inline-scalar-f32@1` installs one admitted CPU model behind the
existing `ModelComputeAdapter`, `PreparedModelInfer`, and ordinary Host Call.
Host preparation supplies a checked `ImportedOnnxResource`, an explicit worker
executable, the configured reviewed ORT library, and finite backing grants to
`hosted_native_model::prepare_native_model`. The selected capability and native
provider share the same immutable model owner and read custody.

The exact profile supports the reviewed 77-byte scalar F32 ONNX Identity graph,
ORT 1.23.2 CPU sequential execution, one required input/output named `scalar`,
and graph-local names `x`/`y`. All scalar IEEE-754 bit patterns are valid,
including signed zeros, infinities, NaN signs/payloads and subnormals. Selected
Plan inspection retains the exact checkpoint, adapter/profile and admission
identity. The retained admission checks the actual runtime, provider, precision
and artifact against those selected Plan facts.

I/O is already materialized in fixed inline scalar storage. The selected Back
profile explicitly names that realization. It requests only the model-inference
Host Call and does not advertise a tensor-storage read authority. A tensor
reference carries no implicit read permission; it receives a typed refusal
before native execution. This profile never opens ambient storage for inputs.
A separate admitted tensor-read realization is needed for resource-backed I/O.

The worker retains #6208's authoritative finite mapping ledger and enforced
growth boundary. Parent canonical output, metadata readers, pipe frames and
output capacity are prepared before Play. The ordinary calling-thread Play
proof counts allocation/reallocation requests and checks unchanged kernel value
capacity. Native call-capacity exhaustion returns a refusal with no output.

The development lifecycle uses the existing kernel's pending Host Call custody,
not a model scheduler. One fixed transport request/reply slot belongs to the
exact request and prepared worker. Polling keeps ordinary operator cancellation
responsive; cancellation revokes publication before closing pipes, killing and
reaping that worker. Global operator cancellation retires the selected model
owner before the kernel clears its request table. Late/stale replies cannot
become outputs. A fresh worker requires fresh host preparation; Play never
silently reloads a lost, cancelled or exhausted session.

Completed Plays may retain the same already warmed, immutable preparation.
Runtime/model/precision/provider/profile/admission identities remain fixed;
this is not a cross-artifact cache. Normal owner retirement explicitly unloads
native dependencies. Parent pending canonical input and two metadata readers
are prepared before Play; native framing remains fixed9/8bytes with one-byte
acknowledgement. The preparation pipe handshake has one30-second Host deadline,
including Bootstrap, Ready and final admission ACK. That bounds pipe waiting,
not arbitrary filesystem/spawn/kernel stalls or native execution time.

The profile advertises cancellation and no model cache. It does not claim
graceful ORT internal shutdown, physical residency, hard real-time execution,
or interruption at a particular native CPU instruction. Native adversarial
proof pauses the actual prepared worker and observes its queued request before
normal operator Stop; that is pending-operation cancellation, not a native
computation performance measurement.

Run the repository proof with the explicitly configured installed runtime:

```sh
CONDUIT_ORT_LIBRARY=/absolute/path/to/reviewed/libonnxruntime.so.1.23.2 \
  cargo +stable xtask check model-native-inference
```

The entrance checks typed scalar eligibility, executes actual Source/Plan/Play
for finite and IEEE-754 boundary payloads, and verifies parent storage behavior.
It also checks unsupported runtime/backing admission and an ungranted tensor
reference against the production prepared Host Call owner. A single remaining
native invocation must still succeed after reference refusal, then refuse on
capacity exhaustion. That negative is a direct Host Call control; positive
cases separately run the complete production kernel.

The entrance also executes strict pending-input/refusal controls, ordinary
controlled Value Plays and actual native retained-worker reuse/cancellation/loss.
Native worker identity, runtime/model digests, separate Play identities and raw
outcomes remain explicit. Fixture-provider controls do not establish ORT execution.

Normal CI compiles the installed-runtime tests and runs ordinary profile units.
The supported entrance explicitly selects the installed-runtime tests; it cannot
report success by leaving them ignored. No model/runtime is discovered or
downloaded, and default/portable builds retain the optional dependency boundary.
