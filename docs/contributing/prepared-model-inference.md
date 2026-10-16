# Prepared named model inference

Run the development proof through the supported repository entrance:

```console
cargo xtask check model-inference
```

This check covers immutable model metadata admission, typed named tensor I/O,
provider eligibility and finite resource admission, and authored Source through
Plan and the production kernel's admitted model Host Call. The numerical
reference profile loads its two-by-two Float32 weights and bias from the admitted
24-byte resource: input `[2, 3]` produces `[5, 1]`. The multiple-port proof uses
named inputs in reverse order to verify name-based binding. These are small
reference profiles, not speech models or quality demonstrations.

The model signature is validated during preparation. Each invocation uses the
same prepared named-I/O validator and borrows the canonical input and provider
reply. The fixed reference-linear provider prepares output encoding storage
before Play. Its complete prepared Source/Plan/Play test asserts zero allocation
or reallocation requests during Play, including output delivery. This measures
allocation requests, not zero memory use; preparation and fixture construction
are outside the measured interval. The optional Burn provider uses the same
adapter interface but retains framework materialization and has no zero
allocation claim.

Provider replies must retain declared names, dtypes, dimensions, axis roles,
identities, units and content digests. An advertised fixed output template cannot
silently rewrite conflicting provider metadata. Actual input byte counts remain
exact for each invocation; repeated smaller inputs cannot shrink the immutable
admitted maximum. Partial or failed output writes cannot publish a mixed bundle.

The checks distinguish malformed input, provider loss, reported timeout and
cancellation. A reported provider timeout does not prove deadline preemption.
ONNX tests validate the bounded metadata/signature import profile and reject
unsupported metadata or external data; they do not execute ONNX. Actual ORT Host
realization remains separate work under #5220. No model is fetched automatically.

These checks prove development behavior at the checked source. Stable acceptance
requires the normal automated integration and release train.
