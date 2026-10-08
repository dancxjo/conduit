# Common prosodic targets and observed acoustic evidence

Additive Speech components for #5212, pinned to Tongues
[b03702798db5a5e7d174278ac2fc1233f171b02e](https://github.com/dancxjo/tongues/tree/b03702798db5a5e7d174278ac2fc1233f171b02e).
These extend the existing intent ownership; they are not another utterance IR,
execution Plan, model vocabulary or renderer. The integration owner composes
ordinary typed components with the original prepared common carrier.

Five target components keep their canonical static Types below 65,536 bytes:
`SpeechTimingPitchTargets`, `SpeechLinguisticRateTargets`,
`SpeechIntensityTargets`, `SpeechFormantTargets`, and
`SpeechVoiceQualityTargets`. The target components retain exact source references,
a declared linguistic scope and an abstract elapsed-second anchor. A scope label
alone does not establish utterance/revision membership; the common prepared owner
must admit those relations. The anchor grants no Host clock relation.

Existing `SpeechSegmentProsodyIntent`, exact duration/fundamental-cycle/relative
intensity owners, `SpeechSpec<T>` and `SpeechEvidenceProvenance` remain unchanged.
Known, unknown, unspecified, not-applicable, variable and gradient remain distinct.
In particular, preparation does not replace an uncertain value with zero.
Formant center and bandwidth have **independent** specifications and trajectories.
Independent controls may overlap; no phone-sized partition is imposed.

## Pinned field mapping and declared units

| Tongues field | Speech component meaning and current capability |
| --- | --- |
| ProsodyTrack.pitch / AcousticFrame.f0_hz | Positive exact Audio Hz or existing positive Speech cycle, with executed reciprocal/field bridge. Known targets use actual Audio step/linear evaluation; observed F0 has its own six-state Hz specification. A source convention using numeric zero for unvoiced F0 needs an explicit interpretation profile; preserve it as an unsupported original field rather than guess. |
| ProsodyTrack.energy / AcousticFrame.energy_db | Separate exact relative amplitude, power and reference-bearing dB. Audio amplitude/power trajectories support step/linear; reference-dB curves initially support exact step selection. Existing Audio exact-power-of-ten conversion remains the bounded numeric capability; general logarithms remain open. Missing upstream reference metadata is retained explicitly as unsupported original material, never inferred as SPL or watts. |
| Prosodic duration / break.duration_s | Existing exact rational seconds plus step duration curves. Value duration and curve's elapsed-time span are separate meanings. Boundaries/break identities remain with the existing/root boundary owner. |
| ProsodyTrack.speaking_rate | Exact nonnegative syllables/second with explicit linguistic scope and source refs. Positive rate has an executed exact seconds/syllable reciprocal. Zero rate is valid canonically but refuses reciprocal. Step curves preserve every Spec state; general linear rate curves explicitly refuse this first capability. |
| Formant.index / hz / bandwidth_hz | Bounded declared index; independent six-state exact Hz center/bandwidth. Known control trajectories require the frequency domain, separately from coefficients/sample periods; no sample-rate/Nyquist bound is imposed on the authored quantities. |
| AcousticFrame.voicing_probability / periodicity | Separate fields of exact [0,1] numerator/denominator, with Source-owned numerator≤denominator law. Independent step trajectories, never amplitude aliases. |
| AcousticFrame.harmonicity | Original feature material, original Spec state, pinned source profile and explicit limitation. Tongues does not declare the unit. Numeric interpretation explicitly refuses; no ratio/dB assumption. |
| AcousticFrame.spectral_centroid_hz | Nonnegative full-U64 exact Hz fraction, allowing zero independently of positive F0. |
| AcousticFrame.spectral_tilt_db_per_octave | Signed I64/U64 exact dB/octave slope with declared amplitude20/power10 convention. Executed delta for declared offsets -1,0,+1 octave; slope numerator MIN × -1 and wider offsets refuse before arithmetic. This is not a log2 frequency-ratio estimator or a DSP filter implementation. |
| AcousticFrame.zero_crossing_rate | Exact nonnegative crossings/second with explicit counting convention. No inferred normalized-per-sample convention. |
| AcousticFrame.vectors | Original bounded finite F32 sequence, kind, source profile, unit declaration and provenance. No MFCC/embedding/model interpretation is inferred. `require_interpreted_measurements` refuses the unimplemented general interpretation capability while leaving the original evidence inspectable. |
| AcousticFrame.span / CurvePoint.time_s | Exact abstract anchored seconds for admitted execution; optional original `SpeechSegmentSpan` retains upstream float-second/frame material. No implicit conversion or clock relation is asserted between these carriers. |
| CurvePoint.confidence / Spec.gradient | Existing Speech gradient confidence and six-state specifications. Frame-level confidence is itself a Spec so absent upstream metadata can remain unknown/unspecified rather than acquire an invented number. |
| ProsodyTrack.breaks / labels | Owned by the existing/root boundary/label/common-intent mapping. This component does not replace them or claim their integration. |

`unsupported_fields` retains a named original `SpeechSpec<SpeechFeatureValue>`,
source profile and limitation for material that cannot yet be interpreted in the
canonical unit/reference profile. This is an explicit unsupported channel, not
permission to describe the corresponding typed field as a successfully converted
measurement. No Tongues float importer is implemented by this checkpoint.

## Executed boundaries and custody

Source performs duration/cycle/amplitude field projections, syllabic reciprocal,
SingleOctave tilt arithmetic, temporal comparison, coverage/endpoint decisions,
step-profile and Audio role/domain checks. Canonical fractions retain full
meaningful U64 domains. Exact temporal comparison has a separately named U32
operand profile: each cross-product fits U64; no fractions are reduced or clamped.

Rust traverses at most 16 authored segments and copies exact selected fields.
Source owns ordering, span validity and half-open/right-continuous/final-endpoint
coverage decisions. Gaps and foreign anchors refuse. Typed step profiles refuse
linear rather than approximate it. Full numeric pitch/intensity/formant curves
reuse Audio's actual separately bounded evaluator and its original exact receipts.

Full original Native input admission precedes every operation. Immutable receipts
retain original typed input/frame, query, selected full segment, every exact
Source program/input/raw output and admitted final result. Oversized semantic
carriers do not become Source runtime ports: arithmetic operates on private
smaller admitted projections while the complete original stays in the receipt.
These are allocating preparation seams, with no Flow, Host/Plan, PCM or real-time
claim. Unsupported profile refusals do not produce substitute values.

There is a separate architectural defect in generic Known payload parent-law
retention. The adjacent
[reproducer](tests/common_acoustic/SPEC_KNOWN_LAW_REPRODUCER.md) documents the
isolated checked Source and executed Native observation. Preparation explicitly
re-admits a Known probability through its original `SpeechUnitInterval` owner;
the generic wrapper alone is not claimed to prove that nested law. No kernel or
checker guard is weakened here.

## Measurement and validation boundary

Representative actual Native constructor→encode→decode equality passes for each
factored component; their static Type canonical bytes also decode under unchanged
defaults. Static Type / representative all-Unknown value bytes:

| Component | Type bytes | Value bytes |
| --- | ---: | ---: |
| Timing/pitch | 26,985 | 27,544 |
| Linguistic rate | 44,401 | 44,904 |
| Intensity | 38,880 | 39,414 |
| Formants | 27,449 | 27,935 |
| Voice quality | 38,763 | 39,338 |

Evidence has a 22,150-byte static Type and a 24,009-byte representative value
with retained unsupported energy, harmonicity, vector and original-span material. These are not 2KiB Flow
port claims. A discarded whole-prosody grouping measured 157,959 Type bytes and
158,706 value bytes and actually Native-roundtripped in the local 262,144-byte
structured canonical envelope; it was factored to avoid relying on that large
carrier at other Type/frame boundaries. No limits were changed.

The standalone component gate uses actual original Speech Source dependencies,
exact checked Audio owners and canonical Language bindings, and includes these
production adapters/tests. It executes original Speech→Audio→declared 8/16/48k
projection, Source replay, independent u128/i128 references, zero/overflow and
unsupported profiles, all six step states, independent overlapping controls,
right-continuity/final endpoints/gaps, foreign anchors, reversed/coincident/internal
overlap, typed forged Known probability, observation/original-material custody,
role/domain refusals and actual Native sizing/roundtrip. It also executes the
isolated generic Known-law reproducer. No full Speech/DSP build or production
common-carrier join is claimed. The integration owner owns that gate and the
binding-only patch; no root build, manifest or facade is changed by this commit.
