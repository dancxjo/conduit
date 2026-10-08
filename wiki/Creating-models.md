# Creating models: teach Conduit to speak

**Proposed development journey for #5205–#5207.** Portable training semantics
already exist. The Burn back and ConduitVoice are being implemented against
this chapter. Proposed commands/examples are promises to test, not claims of
working software. Verified examples will carry their actual proof.

This chapter is for a Rust contributor who understands ordinary plots, plans,
and plays. Build, train, evaluate, checkpoint, offer, and use one learned model:

```text
committed linguistic facts → ConduitVoice → FARGAN features → FARGAN → PCM → Mask
```

Conduit owns meaning, identity, planning, bounds, and evidence. A selected back
owns numerical realization. Burn supplies tensors, autodiff, optimizer internals,
and device kernels inside that Rust back.

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
[hosted authoring contract](../targets/std/model-authoring.md) and existing
[model-compute semantics](../semantics/ai/src/model_compute.rs).

## Implement the model once

ConduitVoice v0 uses phone embeddings, a small convolutional encoder,
duration/pitch/energy/voicing heads, explicit length regulation at 100 Hz,
and a compact acoustic decoder with an exact FARGAN projection. Configuration
is versioned data. One model definition serves training and inference;
inference needs no optimizer or trainer-only modules.

Its training forward participates in the existing
[training semantics](../semantics/ai/src/training.rs). The tiny tutorial uses
the same model with a smaller configuration. A tiny adjacent regression model
demonstrates generic reuse without replacing the voice walkthrough.

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

The proposed repository proof entrance is:

```sh
# Proposed: not yet runnable.
cargo xtask prove model-authoring --output work/model-authoring
```

Public workflows use `conduit run` with checked training/evaluation/inference
plots and prepared host/body resource configuration. Runnable CPU fixture,
full local training, and CUDA commands will be added as those paths pass proof.
Authored plots contain no paths or device bindings. Host preparation owns them.

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

These are proposed proof names, promoted only when implementation/evidence exist.

| Promise | Proof |
|---|---|
| Honest bounded authoring | `authoring_contract_refuses_invalid_profiles` |
| Real CPU autodiff learns | `burn_cpu_loss_decreases` |
| Cancellation retains parameters/optimizer | `cancelled_step_retains_parameters_and_optimizer` |
| Evaluation is read-only | `evaluation_preserves_training_state` |
| Safe reload preserves inference | `checkpoint_reload_preserves_inference` |
| Durable objects precede cursor | `checkpoint_failure_retains_durable_cursor` |
| Fresh runtime resumes exact work | `fresh_runtime_resumes_recorded_batch_cursor` |
| Natural targets match reference | `natural_speech_reference_feature_differential` |
| Recorded features reconstruct | `recorded_features_authored_fargan_reconstruction` |
| Ordinary planner selects voice | `planned_conduitvoice_feature_contract` |
| Interchange refuses incompatible meaning | `model_interchange_preserves_contract_and_provenance` |
| Docs commands stay executable | `model_authoring_documented_commands` |

Cheap CPU proof belongs in CI. Corpus training, CUDA, complete FARGAN synthesis,
and attended listening retain separate proof classes. FARGAN readiness and
weight redistribution permission remain dependencies; this chapter grants neither.
