# Bounded ONNX import preparation (development)

`onnx_metadata::inspect_onnx` examines supplied immutable bytes during preparation.
`imported_onnx::ImportedOnnxResource::bind` retains an existing
`AdmittedModelResource`, including its immutable storage, signature and admitted
read grant. Neither operation opens a path, contacts a network, downloads a model
or invokes an inference provider.

The ONNX wire field reference is
[onnx-ml.proto at 18a4972ac6482703dc105de8ade85574207bd241](https://github.com/onnx/onnx/blob/18a4972ac6482703dc105de8ade85574207bd241/onnx/onnx-ml.proto).
The parser is a bounded metadata reader, not a general protobuf implementation
or an ONNX operator/type/topology checker.

## Identities and bounds

The graph SHA-256 identifies raw file bytes. Existing model content and resource
descriptor identities retain their own domain-separated meanings. The computed
sidecar identity binds the admitted resource descriptor, graph SHA-256, semantic
signature identity, graph name and ordered mappings in both directions.

Graph-local names map explicitly to existing `ModelSignature` ports. Local model
names do not become universal semantic port names. Input and output counts must
match exactly, every mapping is unique within its direction, and every port is
required. A matched port has one exact dtype, rank and finite dimension envelope.
Fixed graph dimensions require that exact dimension in the semantic signature;
symbolic dimensions use its finite bounds. Repeated symbols require consistent
bounds across inputs and outputs. `validate_named_inputs` and
`validate_named_outputs` additionally check actual dimensions against the
semantic signature and require shared symbols to have one actual size across
the corresponding input/output values. These are preparation validators.

Preparation accepts at most 16 MiB of graph bytes, eight operator-set imports,
32 total input/output ports, eight dimensions per port and 128 UTF-8 bytes per
name. Each visited protobuf message has a 65,536-field traversal limit; nesting
depth is fixed by the supported schema paths. All preparation allocations are
bounded by these limits. There is no Play implementation in this component.

The initial profile accepts dense i8/u8/i16/u16/i32/u32/i64/u64/f32/f64 tensor
ports. Scalar rank zero, unknown dimensions, zero dimensions, string/bool/half
tensors, optional/sequence/map/sparse/opaque port types, node attributes,
multi-device node configurations, sparse initializers, training graphs and
local functions refuse. Every top-level initializer is inspected for external
storage. Attributes and other nested graph/tensor carriers refuse entirely so
they cannot hide an uninspected external tensor reference. No external data is
opened, including otherwise valid relative paths.

## Proof boundary

Data owns one bounded `NamedTensorBundle` over existing `TensorValue` values.
Names, dtypes, dimensions, axis roles and content/resource descriptors retain
their existing owners. Model signature validation matches semantic port names;
sidecars translate those names to checkpoint-local graph names. Canonical Native
encoding retains the bundle order and is subject to Core's 262,144-byte encoded
value bound, independently of the model file byte bound.

The development `model/infer` Fore uses this carrier in both directions and a
required typed semantic `ModelSignature` startup parameter. The owner catalog
imports checked Source refinement metadata. A compiler regression additionally
checks those refinements on explicit concrete startup expressions, including
shape-only aliases. A tiny non-speech authored Source checks and expands to one
opaque model Gear. It contains no provider, artifact path or device selection.
Normal planner candidate admission now matches the exact typed semantic
signature property. Planning fixtures select the matching Back/artifact,
refuse missing or stale properties and verify the sealed Plan. These fixtures
execute no inference. The catalog is not yet installed in the stock Host
catalog; genuine model compute/resource admission and an execution Back remain
unfinished.

`ModelComputeRequirement` now declares a finite dtype set, and an offer must
support every required input/output dtype. Existing callers use this field;
the invocation payload still needs migration from its old single tensor onto
the canonical named bundle before ordinary model execution is delivered.

Focused native tests cover metadata and sidecar binding, wrong identities,
counts, names, rank, dtype, static dimensions, malformed wire data and external
data refusal. The main imported fixture contains one Add node and four locally
authored inline bias values; these tests inspect its metadata without executing
it. Separate symbolic-shape fixtures test only metadata. None is an inference
proof.

Issue #5691 remains unfinished: exact Source → Plan → Play evidence is still
required. Genuine model-compute Back
selection is tracked in #5692, canonical named tensor invocation migration and
numerical multi-input/output execution in #5693, and actual ORT realization in
#5220. No current helper result establishes those capabilities.
