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
bounds across inputs and outputs. This checks envelopes; invocation-time symbol
equality still needs validation against actual tensor dimensions.

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

Focused native tests cover metadata and sidecar binding, wrong identities,
counts, names, rank, dtype, static dimensions, malformed wire data and external
data refusal. The synthetic metadata fixture deliberately tests metadata only;
it is not an executable model or an inference proof.

Issue #5691 remains unfinished: ordinary Source typing and exact
Source → Plan → Play evidence are still required. Genuine model-compute Back
selection is tracked in #5692, canonical named tensor invocation migration and
numerical multi-input/output execution in #5693, and actual ORT realization in
#5220. No current helper result establishes those capabilities.
