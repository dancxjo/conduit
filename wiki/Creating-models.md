# Creating models: teach Conduit to speak

**Development journey for #5205–#5207.** The hosted Burn library foundation is
implemented and tested. The generic regression journey also has an ordinary model-work Plot/HostCall
path. ConduitVoice and corpus preparation remain proposed. Proposed examples are promises to test;
the adjacent regression proof does not establish the voice journey.

This chapter is for a Rust contributor who understands ordinary plots, plans,
and plays. Build, train, evaluate, checkpoint, offer, and use one learned model:

```text
committed linguistic facts → ConduitVoice → FARGAN features → FARGAN → PCM → Mask
```

Conduit owns meaning, identity, planning, bounds, and evidence. A selected back
owns numerical realization. Burn supplies tensors, autodiff, optimizer internals,
and device kernels inside that Rust back.

## Shared speech prerequisite

The speech portions of #5204, #5206, #5207 and #5208 depend on the common
linguistic contract in #5212. Preserve checked Unicode IPA, separate phoneme
and phone occurrences, exact Variety/inventory/text revisions, syllables,
word correspondence, quantity targets and supplied observations. Checkpoint
vocabulary is a terminal projection of that original carrier; display IPA or
compact renderer labels cannot reconstruct it.

Run `cargo +stable xtask check speech-ipa` and
`cargo +stable xtask demo native-speech --output work/carried-speech` for the
shared-carrier conformance and retained offline PCM specimen. See
[Shared speech acoustic custody](../docs/design/speech-acoustic-custody.md)
for evidence boundaries. The versioned training and inference lexical maps
share one implementation. Their synthetic checkpoint metadata has no trained
weights, and the acoustic model terminal explicitly refuses with
`NoAdmittedAcousticBack`. The formant specimen does not establish the proposed
ConduitVoice/FARGAN journey below. Accepted integration evidence is required
before the governing speech prerequisite can close.

## Run the tiny CPU model first

The repository's synthetic regression fixture learns `y = 2x + 1`; it uses
no downloaded corpus, credentials, or GPU. Its source and generated weights
use the repository MIT license (see `LICENSE`). Fixture dataset/build identities are explicitly
synthetic, not evidence about an external dataset or speech quality.

From a repository checkout with its supported Rust toolchain, choose a fresh
output directory for each run:

```sh
cargo xtask prove model-authoring --output work/tiny-model-library
cargo xtask prove model-authoring --plot-journey --output work/tiny-model-plot
```

The first command runs the public Burn adapter contracts. The second checks
`plots/model-authoring/main.conduit`, expands its `model/work` Kind, plans
against the prepared std Host offer, and runs actual installed Kernel HostCalls.
The external Fore carries bounded `ModelWorkRequestBytes` and
`ModelWorkReplyBytes` envelopes, each at most 1,048,576 bytes; the decoder
checks the enclosed typed requests before model work.

The regression model's `x` and `y` Fores contain F32 tensors with Batch ×
Feature axes: 1–8 rows and exactly one feature, at most 32 bytes per tensor.
The small training batch is `x = [-1, 0, 1, 2]`, `y = [-1, 1, 3, 5]`.
Its recipe uses CPU Flex, AdamW, learning rate 0.05, weight decay 0,
gradient clipping 10, seed 42, and 80 bounded steps. The definition and
host preparation remain Rust code through the public authoring seam; the
Plot receives admitted requests rather than inventing a model declaration syntax.

Inspect `manifest.json`, `stdout.log`, and `stderr.log` in the output directory.
Success requires `passed: true` and `source_unchanged: true`. The Plot test
reports measured loss, checks loss reduction, exact inference after reload,
fresh-runtime resume from step 80 to 81, and the requested/completed HostCall
records. The hosted library producer also retains immutable resume and
inference artifacts under `artifacts/`. Existing output directories refuse
rather than overwrite evidence.

Wrong tensor geometry refuses as `InvalidTensor` before numerical work;
resource estimates outside admitted bounds refuse as `ResourceBound` or
`InvalidDescriptor`, depending on the admission boundary. Correct the shape
or explicitly prepare a new bounded model profile; increasing a caller's
request does not increase admitted resources. Refusal preserves committed
model and optimizer state. CPU `products` CI exercises the adapter contract
suite; `hosts-std` enables `burn-model` and exercises the ordinary Plot journey.
These are deterministic CPU proofs. Voice training, external corpus licensing,
held-out speech quality, CUDA equivalence, and physical-host acceptance each
need their own evidence.

## Choose an on-ramp

A host can realize meaning at different levels. A whole-model Rust/Burn back
may implement bounded acoustic prediction. A checked plot back may compose
duration, prosody, and spectral prediction. Lower-level numerical backs may
realize authored tensor operations, as the existing FARGAN work does. Another
framework, service, or future mechanism can enter at any exact contract it
truthfully satisfies. Burn is one on-ramp, not the model ontology.

Every substitution still requires the same kind and fore, exact input/output
meaning, finite bounds, current offers, authority, and compatible numeric
profiles. Selecting a smaller seam does not erase identity or provenance.
Native plots do not have to reproduce Burn's autodiff machinery. An opaque
provider does not have to expose a portable gear for every internal operation.

Off-ramps are equally explicit: export inference weights, resume bundles,
dataset/derived observations, predictions, and evaluation evidence for use by
another tool. Local representation and external identity stay distinct from
Conduit meaning. Re-import checks format/schema, content identity, compatibility,
provenance, and authority; an exported checkpoint is data, not executable
authority. Numerical similarity alone does not make two artifacts identical.

## Define a model contract

Keep architecture/config, model artifact, mutable training state, immutable
checkpoint, and runtime realization distinct. A filesystem path locates data;
it is not portable dataset identity. Framework tensors/autograd graphs remain
host-private rather than becoming portable Conduit values.

ConduitVoice accepts bounded committed English phone occurrences with stress,
context, and syllable/word/phrase boundaries. Available explicit prosody remains
input; missing prosody is not zero. Unsupported phones, languages, shapes, and
profiles refuse rather than approximate silently.

Output is the exact selected FARGAN conditioning contract: 20 features per
10 ms epoch plus admitted period. Features 0–17 follow its spectral conventions;
18–19 follow its pitch/correlation conventions. Cadence, window/history,
startup, delay, scaling, period range, and boundary rules are part of that
profile. An arbitrary mel spectrogram cannot substitute.

The Rust authoring seam declares architecture/version, bounded signatures,
parameter groups, trainable/frozen policy, checkpoint schema, supported
precision/device profiles, and a finite resource estimate. See the
[hosted authoring contract](https://github.com/dancxjo/conduit/blob/dev/targets/std/model-authoring.md) and existing
[model-compute semantics](https://github.com/dancxjo/conduit/blob/dev/semantics/ai/src/model_compute.rs).

## Implement the model once

ConduitVoice v0 uses phone embeddings, a small convolutional encoder,
duration/pitch/energy/voicing heads, explicit length regulation at 100 Hz,
and a compact acoustic decoder with an exact FARGAN projection. Configuration
is versioned data. One model definition serves training and inference;
inference needs no optimizer or trainer-only modules.

Its training forward participates in the existing
[training semantics](https://github.com/dancxjo/conduit/blob/dev/semantics/ai/src/training.rs). The tiny tutorial uses
the same model with a smaller configuration. A tiny adjacent regression model
demonstrates generic reuse without replacing the voice walkthrough.

For a complete small implementation, read the executable
[regression model definition](https://github.com/dancxjo/conduit/blob/dev/mechanisms/implementations/burn-model/tests/common/mod.rs)
and its [retained journey](https://github.com/dancxjo/conduit/blob/dev/mechanisms/implementations/burn-model/tests/retained_journey.rs).
It learns `y = 2x + 1` with a single Burn linear layer, an explicit bounded
signature, one parameter group, and mean-square objective. Its synthetic dataset
and identities are fixtures. Copy the authoring pattern, then supply your own
semantic contract and truthful preparation evidence.

The following Rust helpers compile against the public hosted library seam:

```rust,no_run
use conduit_burn_model::{BurnAdapter, BurnModelDefinition, DeviceRequest, Error,
    OptimizerRecipe, PreparedBurnModel, TrainingContext};

fn prepare<D: BurnModelDefinition>(definition: D) -> Result<PreparedBurnModel<D>, Error> {
    PreparedBurnModel::initialize(definition, DeviceRequest::Cpu,
        OptimizerRecipe { learning_rate: 0.001, weight_decay: 0.01,
            gradient_clip: 1.0, seed: 42 })
}
fn start<D: BurnModelDefinition>(prepared: PreparedBurnModel<D>, context: TrainingContext)
    -> Result<BurnAdapter<D>, Error>
{
    // Host preparation publishes prepared.weights(), then constructs the existing
    // artifact/session identities from prepared.content_identity().
    BurnAdapter::from_prepared(prepared, context)
}
```

`PreparedBurnModel` retains the initialized module so creating its content-bound
artifact does not initialize a random model a second time. Admission checks that
its SafeTensors bytes match the base artifact's digest and extent. Host preparation
still owns durable publication and truthful dataset/build/resource evidence.

An exported checkpoint has a separate inference entrance. Its context contains
artifact and host runtime evidence, without any dataset, training session, or
optimizer recipe. The loader verifies the descriptor and SafeTensors members:

```rust,no_run
use conduit_burn_model::{BurnModelDefinition, DeviceRequest, DirectoryCheckpointStore,
    Error, InferenceBurnAdapter, InferenceContext};
fn reload<D: BurnModelDefinition>(definition: D, context: InferenceContext,
    store: &DirectoryCheckpointStore, checkpoint: &[u8; 32])
    -> Result<InferenceBurnAdapter<D>, Error>
{
    InferenceBurnAdapter::load(definition, DeviceRequest::Cpu, context, store, checkpoint)
}
```

The capstone call is **proposed**, pending ConduitVoice and its exact FARGAN gate:

```rust,ignore
let definition = ConduitVoiceDefinition::new(ConduitVoiceConfig::tiny());
let prepared = prepare(definition)?;
// Publish the base artifact, prepare checked speech data and construct context.
let mut model = start(prepared, context)?;
```

## Prepare truthful data

Use [LJ Speech 1.1](https://keithito.com/LJ-Speech-Dataset/), a single-speaker
English corpus whose publisher identifies text, audio, and annotations as
public domain. Keep the full corpus outside Git. Retain source/version/content
identity, permissions, speaker/session scope, transcript provenance, original
audio geometry, preprocessing, and stable split membership.

Resampling 22,050 Hz audio to 16,000 Hz creates a derived observation. Pinned
Montreal Forced Aligner English ARPA alignment is derived evidence, not committed
linguistic truth. Record tool/model/dictionary/mapping identities and attribution;
reject unsupported mappings. Group duplicate transcripts and overlapping source
segments before deterministic split assignment. Fit normalization on training
data only. The checked excerpt retains alignment and requires no aligner in CI.

**Before acoustic training**, extract reference features from recorded speech
and reconstruct through existing authored FARGAN. Compare against the pinned
upstream analysis reference; retain original/reconstructed WAVs and listening
evidence. Fix or report a profile mismatch before proceeding. The formant
spectral approximation is not the training target.

## Write a bounded recipe

Name model/config, fresh initialization or base checkpoint, dataset/splits,
batch/shuffle seed, optimizer/scheduler, clipping, weighted objectives,
step/work bounds, evaluation/checkpoint cadence, and reproducibility profile.
CPU/CUDA is realization selection unless a numeric difference is explicitly
material. An exact unavailable device request refuses.

Admission includes committed/candidate parameters and optimizer state,
autodiff/temporary storage, batch input/output, queueing, checkpoint bytes, and
finite work. A seed is not a promise of cross-device bitwise reproducibility.

## Train as ordinary Conduit work

The hosted library foundation has a runnable repository proof entrance. It
executes the adjacent regression contract tests, not the proposed voice path:

<!-- model-authoring-command -->
```sh
cargo xtask prove model-authoring --output work/model-authoring
```

Verify the documented command itself with
`cargo xtask prove model-authoring --documented-command --output work/model-authoring-docs`.
The producer extracts the marked command, substitutes only the host-local evidence
directory, executes it through `cargo xtask`, and retains document identity and logs.

Run the generic Plot journey with:

```sh
cargo xtask prove model-authoring --plot-journey --output work/model-authoring-plot
```

The [authored Plot](https://github.com/dancxjo/conduit/blob/dev/plots/model-authoring/main.conduit)
accepts canonical model-work byte envelopes through public external Fores. The
prepared Host offers the model session; ordinary planning selects it, and the
installed kernel dispatches each operation through its admitted Host Call.
The regression journey trains, evaluates, checkpoints, exports, reloads for
inference, restores a fresh training runtime, and advances the recorded step.
It checks the actual Kernel request/completion records and exact inference output
across reload. The byte envelope is a transport codec; its decoder retains the
existing semantic training requests, tensors and receipts as payload authority.

The CPU fixture uses F32 and constant-rate AdamW. Host preparation owns the
compiled definition, checkpoint store and device. The Plot contains no filesystem
or device bindings. Normal public CLI setup for arbitrary model families and the
voice composition remain separate work.

Inspect Plan/Play/Sign evidence as well as console progress: exact selected
back/device/precision, admitted bounds, dataset/batch/objective identities,
generation, consumed work, metrics, and terminal outcome.

## Cancel without a half-step

Compute candidate parameters and optimizer state without replacing committed
state. Publish both through the semantic state boundary only after validation
and successful work. Cancellation/refusal/failure retains the prior generation.
Output pressure cannot apply an update twice. Candidate snapshots count toward
the admitted memory bound.

## Evaluate components

Evaluation updates neither parameters, optimizer, random cursor, nor running
statistics. Report duration error, voiced/unvoiced performance, voiced pitch
error, per-feature reconstruction error, and combined validation objective.
Retain held-out formant→FARGAN, recorded-features→FARGAN, and predicted-features
→FARGAN WAVs. Record intelligibility/pronunciation listening separately from
loss. Measure first-audio latency, steady RTF, model bytes, and peak memory.

## Checkpoint, stop, and resume

Inference weights use SafeTensors with a versioned Conduit descriptor. Resume
bundles additionally retain optimizer/scheduler/scaler state where supported,
randomness, batch cursor, generation, and exact recipe/data/config identities.
Inference export is an off-ramp that omits resume-only machinery.

Publish and sync immutable objects before atomically advancing the durable
checkpoint/cursor reference. In-memory committed progress and the last durable
resume point remain distinct. Best/latest reference immutable snapshots. A write
failure cannot advertise an incomplete bundle. Reload validates content/schema
and compatibility; stale recipes, corruption, and unsupported schemas refuse.

The tiny proof stops, creates a fresh runtime, resumes the recorded cursor,
advances work, exports, reloads, and compares inference within declared tolerance.

### Inspect the tiny model's durable restart

The tiny CPU command above performs the save/stop/resume/export/reload sequence
through the public adapter and durable store. Open
`work/tiny-model-library/artifacts/journey.json`: its versioned
`conduit.fixture/model-authoring-journey@1` record reports the durable resume
step (40), final committed step (80), before/after loss, checkpoint identities,
and `exact_same_device_reload`. The fixture's measured loss decreases from
9.0 to about 0.004; a run must meet its actual test bound rather than copy this
number into a success claim.

The immutable objects live separately under `artifacts/resume/` and
`artifacts/inference/`. Keep each whole store together: the content-derived
JSON descriptor names its weight/model/optimizer blobs. `latest.json` locates
a durable resume identity; the pathname does not grant model authority.
The inference export contains weights and a versioned descriptor, with no
optimizer or training-session authority. Its fixture extent is 928 bytes.

Follow the executable `retained_journey.rs` example: `checkpoint` returns the
exact identity, the original adapter is dropped, a fresh adapter calls
`resume` with that identity and the same data/config/recipe, and training
continues from the next step. After `export_inference`,
`InferenceBurnAdapter::load` receives the model definition, admitted runtime,
store, and exact checkpoint identity; it needs no training dataset or optimizer.
The ordinary Plot command additionally tests the same operations as admitted
`model/work` HostCalls and checks their Kernel request/completion Sign records.
These records are execution evidence, not a claim of cryptographic signing.

A different corpus payload or shuffle recipe, model configuration,
checkpoint schema, or training recipe receives
`IncompatibleCheckpoint` and leaves committed state unchanged. Restore the
matching definition and admitted context; the loader does not silently train
a replacement. Corrupted descriptor/blob bytes and incompatible optimizer parameter IDs/shapes
receive `CorruptCheckpoint`;
recover the known immutable store instead of editing its content-derived names.
The CPU library command runs these refusal tests along with the restart proof.
A fresh output directory replays the demonstration without overwriting a prior
store. The synthetic regression and its restart teach public lifecycle APIs;
#5207's trained voice and held-out speech acceptance remain separate.

## Offer and use the capability

Ordinary artifact/admission machinery loads the checkpoint and offers a bounded
model back. The planner selects it and composes feature epochs with existing
plot-authored FARGAN and audio presentation. No private ConduitVoice runner
bypasses discovery, planning, or kernel execution.

Swap model/back where exact contracts permit. Formant, ConduitVoice→FARGAN,
compatible reference voices, and hosted speech retain separate realization
facts. Neither speech nor model meaning is defined by one backend.

## Make your own

Define semantic inputs/outputs; version architecture/config; implement the Rust
seam; identify data/derived evidence; write a bounded recipe; train/evaluate;
checkpoint; offer an inference back; compose it in plots.

A sensor anomaly detector maps bounded calibrated windows to explicit anomaly
scores. A motion predictor maps admitted pose histories to bounded trajectories
with declared uncertainty. Each can enter through Burn, native composition, or
another back and export evidence through the same explicit artifact boundary.

## Promises and executable proof

These proof cases distinguish hosted-library evidence from the proposed capstone.
The [evidence guide](https://github.com/dancxjo/conduit/blob/dev/proof/model-authoring/README.md) records proof scope and
the separately pinned FARGAN dependency inspection.
[Retained clean-source proof](https://github.com/dancxjo/conduit/blob/dev/proof/model-authoring/evidence/0d8aca837/README.md)
includes real regression resume/inference artifacts, CPU loss improvement, and
separate CUDA training/inference comparisons. It does not establish a voice.

| Promise | Proof | Status |
|---|---|---|
| Honest bounded authoring | `authoring_contract_refuses_invalid_profiles` | hosted library tested |
| Real CPU autodiff learns | `burn_cpu_loss_decreases` | hosted library tested |
| Cancellation retains parameters/optimizer | `cancelled_step_retains_parameters_and_optimizer` | hosted library tested |
| Invalid evaluation identity refuses before work | `invalid_evaluation_batch_refuses_before_objective_execution` | hosted library tested |
| Signature batch bounds fit realization | `signature_batch_bound_cannot_exceed_realization_admission` | hosted library tested |
| Evaluation is read-only | `evaluation_preserves_training_state` | hosted library tested |
| Inference reload needs no training context | `inference_export_reloads_without_dataset_session_or_optimizer` | hosted library tested |
| Nonfinite updates never commit | `finite_loss_with_nonfinite_gradient_never_commits_candidate` | hosted library tested |
| Frozen groups survive resume | `frozen_parameter_group_remains_frozen_after_resume` | hosted library tested |
| Optimizer state belongs to these parameters | `foreign_optimizer_parameter_ids_refuse_even_with_valid_content_digests` | hosted library tested |
| Safe reload preserves inference | `checkpoint_reload_preserves_inference` | hosted library tested |
| Durable objects precede cursor | `checkpoint_failure_retains_durable_cursor` | hosted library tested |
| Fresh runtime resumes next step | `fresh_runtime_resumes_recorded_step_cursor` | hosted library tested; finite corpus cursor has a separate fresh-Host Plot proof |
| Snapshot staging is exclusive | `BurnAdapter::checkpoint` / `export_inference` compile-fail examples | hosted library tested |
| Explicit CUDA learns and resumes | `admitted_cuda_training_and_resume` | local RTX 3060 tested; fixture tolerance `1e-3` |
| Natural targets match reference | `natural_speech_reference_feature_differential` | proposed |
| Recorded features reconstruct | `recorded_features_authored_fargan_reconstruction` | proposed |
| Ordinary planner selects voice | `planned_conduitvoice_feature_contract` | proposed |
| Interchange refuses incompatible meaning | `model_interchange_preserves_contract_and_provenance` | proposed |
| Docs commands stay executable | `model_authoring_documented_commands` | hosted command extracted and executed |

The [hosted contract](https://github.com/dancxjo/conduit/blob/dev/targets/std/model-authoring.md) records current v1 limits,
including the explicit resume-device refusal, finite attached-corpus resume,
and the remaining held-out speech/best-checkpoint integration. Its inference off-ramp now reaches the existing
std-host model-compute interface. The generic regression Plot exercises ordinary
planning and installed Kernel HostCalls; voice selection remains proposed.

Cheap CPU proof belongs in CI. Corpus training, CUDA, complete FARGAN synthesis,
and attended listening retain separate proof classes. FARGAN readiness and
weight redistribution permission remain dependencies; this chapter grants neither.
