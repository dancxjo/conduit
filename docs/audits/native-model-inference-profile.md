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

This synchronous profile advertises no interruptible cancellation and no model
cache. Broader warm reuse, cancellation, stale completion and session retirement
acceptance remain #5695. Disposable process cleanup does not claim graceful ORT
session retirement, physical residency, or a real-time execution guarantee.

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

Normal CI compiles the installed-runtime tests and runs ordinary profile units.
The supported entrance explicitly selects the installed-runtime tests; it cannot
report success by leaving them ignored. No model/runtime is discovered or
downloaded, and default/portable builds retain the optional dependency boundary.
