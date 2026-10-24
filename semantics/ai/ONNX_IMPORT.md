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
match exactly and every mapping is unique within its direction. Graph-required
inputs and all outputs remain required; the held optional-input extension below
admits absence only for inspected dynamic-optional inputs. A matched port has one
exact dtype, rank and finite dimension envelope.
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
ports. An explicitly declared empty shape is a rank-zero scalar: exactly one
element, zero axes and exactly its dtype width. An absent shape remains unknown
and refuses. Unknown dimensions, zero dimensions, string/bool/half tensors, sequence/map/sparse/opaque port types, node attributes,
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

The `model/infer` Fore uses this carrier in both directions and a required typed
semantic `ModelSignature` startup parameter. The ordinary catalog and planner
match the signature and admitted execution properties. Checked Source contains
no provider, artifact path or device selection. The former single-tensor
invocation surface has been migrated to canonical named bundles.

`PreparedModelInfer::prepare_imported` retains imported model custody. An exact
Back profile binds the checkpoint, graph/signature/sidecar identities, runtime,
artifact and execution profile, IR/opset ranges and supported operator names.
Admission occurs before provider loading. Operator names alone do not establish
arbitrary graph semantics: the initial reference fixture Backs accept only their
exact checked graph identities. Produced replies retain the admitted runtime.

The selected Plan contains a bounded native inspection property with the real
model Gear's semantic-to-graph port correspondence and selected checkpoint/Back
identities. Its reader checks the selected realization. This is inspectable
Plan data; a rendered Patchbay presentation is not established by these tests.

The imported Add/Sub fixture executes multiple named inputs/outputs through
checked Source, Plan and the production kernel. A separate imported Identity
fixture executes bounded symbolic widths one, two, then one, with exact output
bundles and unchanged kernel value capacity; width three refuses. Wrong graph,
sidecar, signature, operator/opset profile, runtime, shape and memory cases are
covered at their owning boundaries. The supported model-inference suite passes
57 tests across 21 targets at executable source
`996cefe6e85c2991f51a773cfcad5b2494d21bd4`.

This is held, unmerged preparation for #5219. Its original portable scope still
needs acceptance of the rank-zero scalar prerequisite, optional/model-defined inputs
and a numeric finite-only policy when required downstream. Approved external
file admission and rendered Patchbay inspection also remain unsupported here.
The initial profile refuses external tensor references without opening files.
Actual ORT realization remains separate #5220; fixture execution does not prove
ORT execution, current-source CI/Integration, publication or whole #5219 closure.


The #6210 rank-zero prerequisite extends the existing Tensor/ModelSignature
contracts and expands their finite axis bounds to 0..=8. A scalar's empty shape
has an empty product of one element; it is distinct from rank one of length one.
The raw tensor codec, canonical Native bundle and borrowed view check the same
exact dtype width. Checked-in I64 and F32 Identity graphs and authored Source
exercise the ordinary imported preparation and selected Plan inspection in
`model_infer_onnx_scalar`; focused scalar storage and refusal cases accompany
those fixtures. The supported model-inference suite passed 68 tests across 24 targets at clean
Source `9cacd3ca8d8ae486112ba81d1f35b692c25505f1`; the source-bound receipt is in
`docs/audits/evidence/rank-zero-imported-model-20261021`. CI/Integration/merge
remain delivery gates.

The #6302 development candidate adds an explicit `numeric_policy` to each
canonical model tensor constraint: `ieee` preserves existing floating encodings and `finite_only`
requires actual finite F32/F64 values. The policy participates in signature and
selected Plan identity. Shared owned and borrowed validation checks inline values
without rewriting payload bytes; finite-only floating resource descriptors refuse
as unverifiable, rather than opening data or inferring finiteness from a digest.
Integer encodings are finite by construction.

The supported model-inference suite passed 78 tests across 26 targets at
`cbc66b635de725b20e220f5265207f49b49bf275`, including real checked Source, Plan
and Play plus provider-effect/output-publication refusals. Focused strict Clippy
passed at `c65e91c1770d13097ad716a8ab44b6ec48539c04` after two test-only error
assertion style corrections; production objects are unchanged from the full
suite. The refreshed delivery retains those implementation objects. Relevant
Candidate/Integration and merge remain delivery gates. This child does not close
the parent's optional-input, external-data or rendered-Patchbay acceptance gaps.

## Optional-input preparation (#6312)

The importer inspects genuine dynamic optional tensor inputs
only with ONNX IR 8 or newer. Graph-derived presence must match the canonical
model signature exactly. An absent optional input is omitted from the existing
named bundle; present values retain the same tensor checks. Owned and borrowed
symbol tables bind only actual values in the current invocation, and output
names remain mandatory. Nested optional types and optional outputs refuse.
Initializer-defined defaults and approved external tensor data remain unsupported.

All possible input maxima still participate in preparation admission. A zero-byte
actual inference invocation is allowed after that admission; other compute
operations retain their existing positive-input rule. Positive offer capacities,
queue item limits, model/output/working-memory bounds and selected Back identity
remain checked. No new provider authority or Core Option carrier is introduced.

Two immutable Identity fixtures have exact graph-hash-bound Backs: one projects
the required input while ignoring an unused optional hint, and the other returns
an inline constant with both graph inputs optional. Prepared tests cover ordinary
Source/Plan/Play, absent/present/absent and empty/present/empty invocations,
pre-effect refusals, current symbols and allocator effects. They are registered
in `cargo +stable xtask check model-inference --locked`.

The supported suite passed 93 tests across 30 targets, with zero ignored, on
prepared source `330e05561829aed05511e8bebcaa47c200c648bd`; focused strict Clippy
also passed on that source. The [retained proof packet](https://github.com/dancxjo/conduit/tree/7fe3e427aebcd10d67f89dfb51fe521d45e36bb3/docs/audits/evidence/model-optional-inputs-20261023/330e05561)
includes the actual test executables, logs, receipts and earlier failures.
The refreshed delivery preserves the feature code; Candidate and relevant
post-merge Integration remain required on the delivered source. No ONNX checker,
ORT execution, rendered Patchbay proof or parent #5219 completion is claimed.
