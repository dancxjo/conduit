# Hosted Rust model authoring

The [model tutorial](../../wiki/Creating-models.md) owns the complete proposed
ConduitVoice journey. The current `conduit-burn-model` crate proves the hosted
library foundation; it does not yet provide authored training plots or voice synthesis.
Existing portable model/training identities remain authoritative.

## The Rust on-ramp

Implement [`BurnModelDefinition`](../../mechanisms/implementations/burn-model/src/model.rs)
once: a Burn `Module`, initialization, exact parameter-group binding, inference
forward, and objective participation. Return an
[`AuthoringDescriptor`](../../mechanisms/implementations/burn-model/src/contract.rs)
with architecture/version, configuration digest, existing bounded `ModelSignature`,
trainable/frozen groups, checkpoint schema, supported realization profiles, and
committed model/optimizer, candidate, temporary, checkpoint, and work estimates.

The initial realization supports required inline F32 tensor ports, AdamW,
constant learning rate, norm clipping, and per-step seeded randomness. It refuses
unsupported descriptor shapes/profiles. A fully frozen model cannot offer TrainStep.
The small regression definition in
[the shared proof fixture](../../mechanisms/implementations/burn-model/tests/common/mod.rs)
is an adjacent generic example using this same seam, not ConduitVoice.

`PreparedBurnModel::initialize` produces exact SafeTensors bytes and retains the
module. Publish those bytes and construct their content-bound base artifact; then
`BurnAdapter::from_prepared` checks digest/extent and admits the descriptor and explicit `TrainingContext`
against `ModelComputeOffer`, `ModelComputeSession`, and `TrainingSession`.
`DeviceRequest::Cpu` selects Flex; `Cuda(index)` requires the optional CUDA build,
a successful device preparation probe, and a declared model realization profile.
Host preparation supplies actual build/device evidence and verified resource bindings.
The fixture's synthetic resource/build identities are fixture evidence only.

## Other on-ramps

| Host provides | Boundary it must satisfy | Facts retained |
|---|---|---|
| Whole model in Burn or another framework | Exact model signature and model-compute admission | Model/config/checkpoint plus provider/device/numeric evidence |
| Submodel implemented by Rust or a checked plot | Exact duration, prosody, decoder, or other bounded fore | Its own implementation and composition provenance |
| Native plot composition | Checked typed ports, finite plan, selected operation backs | Plot, plan, realization, and resource identities |
| Numerical operation, service, or future mechanism | The particular kind/fore and its admitted authority | Local/external identities and exact supported meaning |

These are independent entry levels. Burn does not become a required universal
registry. An operation back cannot claim whole-model equivalence just because
one tensor looks similar. The existing planner/kernel laws continue to apply;
this library does not implement additional native plot or service backs.

## State and publication

One TrainStep forks model and optimizer state, computes a candidate, checks
finite loss and candidate parameter/optimizer values, actual serialized candidate bounds, generation/work, and cancellation,
then publishes both through the existing semantic commit boundary. Cancellation
and final publication share a gate: cancellation wins before publication; a
publication already holding that gate wins as one complete commit. Failed or
cancelled candidates retain the previous generation. In-flight failure/cancellation
is terminal; unload and initialize/resume a fresh adapter before continuing.
Evaluation uses a valid, detached fork and preserves parameters, optimizer, and cursor.

SafeTensors weights and a versioned descriptor are inference off-ramps. Resume
bundles additionally persist a Burn module record retaining ParamIds, AdamW state,
recipe/seed, generation, consumed work, and the next **step** cursor. The corpus
iterator/batch cursor, shuffled ordering, scheduler/scaler alternatives, held-out
split evaluation, and best/latest policy are still required integration work.
Do not mistake a resumed step counter for a complete corpus cursor.

`DirectoryCheckpointStore` accepts an explicit prepared directory and finite
object/retention bounds. It stages and syncs each immutable object before exposing
its content-derived name; it syncs the bundle before replacing/syncing `latest.json`.
A failed final directory sync returns `DurabilityUncertain(identity)` so a caller
can inspect visibility without claiming a durable receipt. Orphan immutable objects
may remain after interrupted publication; no implicit pruning occurs. The host must
reserve one writer for a store; the library mutex serializes one store instance.

Reload verifies digests, schema, configuration, signature, parameter policy, and
recipe/session compatibility before replacing state. Inference export contains no
optimizer or resume authority. Loading it disables training. Resuming an older
checkpoint into an advanced live state refuses.

Burn 0.22 materializes optimizer records on the process-default device. Resume
explicitly refuses `UnsupportedResumeDevice` if that differs from the selected
device: CPU resume works in CPU-only builds; CUDA 0 resume works in the tested CUDA
build. CPU training/inference still use Flex explicitly when CUDA is compiled.
Supporting CPU resume in that build and other CUDA indices requires an upstream
explicit-device loading seam; silently allocating on CUDA 0 violates admission.

The inference off-ramp must reload with only a model definition, base artifact/
runtime evidence, an admitted store, and the selected checkpoint identity. It must
require no dataset, training session, optimizer recipe, or optimizer state. The
standalone `InferenceBurnAdapter::load` seam is tested by
`inference_export_reloads_without_dataset_session_or_optimizer`. It creates a
valid inference module and retains no optimizer. `BurnAdapter::into_inference`
provides the same boundary after a verified load, dropping training context.

## Artifact off-ramps and re-entry

| Export | Included | Re-entry rule |
|---|---|---|
| Inference checkpoint | SafeTensors, architecture/config/signature/group/schema descriptor | Digest and exact compatibility checks; no training state |
| Resume bundle | Inference weights plus module/optimizer/recipe/step state | Exact session/config/recipe and supported resume device |
| Dataset/features/predictions/evaluation evidence (proposed) | Source/derived content identities, units/profile, provenance | Validate the declared meaning and attribution; never infer it from shape |

With the optional std-host `burn-model` feature,
[`HostedBurnModelComputeAdapter`](src/hosted_burn_model.rs) offers a prepared
single-input/single-output inference checkpoint through the existing
`ModelComputeAdapter` interface. Invocations and results name the exact checkpoint
as well as the base artifact; insufficient resources or a substituted checkpoint
refuse. Reserve the declared worst-case output bound, and check actual input batch
size against the request before invoking Burn. This is the hosted interface, not yet ordinary Plot/HostCall selection.

Admission includes committed/candidate/temporary storage and finite work.
These are cooperative Rust estimates and checked serialization bounds, not a
hard allocator ceiling or hostile-code confinement. Burn tensors/autodiff remain
inside the Rust back. Paths and framework handles never become portable authority.
