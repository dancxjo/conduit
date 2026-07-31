# Bounded learned-model inference

Status: current pre-release contract.

This specification owns reusable learned-model artifact, tensor-schema, and
inference boundaries above `conduit-std`. It does not own speech, vision,
robotics, chat, training, evaluation, promotion, model download, or a second
runtime. Product domains retain their own public values and use an explicit
adapter only when model-level interoperability is useful.

## Identity separation

`LMI-001` A semantic model purpose, model artifact bytes, artifact format,
model graph, provenance, policy/license metadata, input schema, output schema,
runtime implementation, device, resource, provider profile, and installed
provider are distinct identities. Equality of one MUST NOT imply equality or
availability of another.

`LMI-002` Artifact bytes MUST be content-addressed before execution. The first
proof artifact has identity
`sha256:65ecb31b3f30690325f1e29e6bf34e4ca73118b103ccd1d21c32e4db47d7d29c`.
An implementation MUST reject a mismatched artifact, hash, graph, format, or
schema rather than guessing, converting, or downloading.

`LMI-003` A host MAY understand `learned/model-artifact` and `learned/tensor`
while reporting every inference provider unsupported. Contract registration is
separate from provider installation. Custom namespaced model and value types
remain eligible for ordinary exact structural compatibility; this package does
not reserve a universal tensor boundary for product panels.

## Finite schemas and execution

`LMI-004` Every tensor schema MUST bind a finite rank, exact dtype, dimensions,
layout, byte ceiling, batch ceiling, and sensitivity. Dynamic or symbolic
dimensions require their own finite admitted profile and are absent from the
first proof.

`LMI-005` Every inference binding MUST make input bytes, output bytes, retained
bytes, state bytes, work, batching, determinism class, tolerance, pressure,
cancellation, and terminal behavior plan-visible and finite. Provider-private
queues, hidden batching, and hidden recurrent state are non-conforming.

`LMI-006` Missing model, runtime, device, or provider; stale provider evidence;
schema mismatch; resource exhaustion; cancellation; and provider loss are
distinct outcomes. No failure may emit a successful partial result.

`LMI-007` A nondeterministic provider MUST declare its comparison tolerance.
Conformance then compares exact schema, admission, bounds, framing,
cancellation, and terminal behavior plus the declared tolerance. Exact output
bytes are required only for an `exact` determinism class. The first proof is
exact with no tolerance.

`LMI-008` Cancellation is checked by the ordinary production executor and
terminates without inference output. Provider loss is terminal and does not
change the semantic contract or silently select another runtime/device.

## First proof

The one current proof binds:

- a repository-owned 26-byte `conduit-fixed-linear` artifact;
- an exact `i16le`, row-major, `1x4` input schema and `1x2` output schema;
- opset zero, batch one, empty state, 64-byte artifact/input/output ceilings,
  128 retained bytes, and 256 work units;
- `conduit.learned/runtime/rust-fixed-linear` on the exact
  `conduit.learned/device/cpu-reference` resource;
- deterministic output `[35,-3]` through the production exact-plan executor.

The provider is deliberately small. It proves the boundary, not a general
model format or universal inference runtime.

## Owned nodes

- `learned/model/literal` emits the one checked content-addressed model
  artifact.
- `learned/tensor/literal` emits the one checked finite input tensor.
- `learned/infer` binds artifact, schemas, runtime, device, resource, provider
  profile, limits, state, determinism, and tolerance.
- `learned/tensor/inspect` provides a bounded textual proof projection.

The artifact and tensor types are domain-owned values. The ordinary Conduit
plan, scheduler, pressure, cancellation, evidence, and terminal contracts
remain authoritative.

## Conformance

`conformance/c4/learned-inference.json` owns the positive and negative matrix.
Required failures include wrong artifact/hash/format, shape/dtype/layout
mismatch, batch/output/state/work exhaustion, unsupported device/opset, stale
provider, invalid nondeterministic tolerance, cancellation, sensitivity denial,
and provider loss.

`examples/learned-fixed-inference.panel` is the checked standalone proof.
`examples/learned-inference-compose.panel` composes its bounded text projection
with an ordinary standard text operation without changing the learned-model
contract.

## Non-goals

This contract defines no universal intelligence or quality score, framework
class identity, ambient registry ordering, Python object, arbitrary tensor,
implicit model conversion, model/plugin download, training job, dataset,
evaluation, or promotion machinery. Those lifecycle concerns belong to #154.
