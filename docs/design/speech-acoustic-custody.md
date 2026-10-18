# Shared speech acoustic custody

This development slice composes the existing `SpeechUtteranceIntent`, original
phoneme/phone sequences, checked IPA inventory basis, text correspondence and
syllable structure. It introduces no second linguistic IR. The field audit in
[speech-utterance-carrier](../audits/speech-utterance-carrier.md) remains the
complete Tongues mapping, including explicit omissions.

## Execute the conformance

```sh
cargo +stable xtask check speech-ipa
cargo +stable xtask demo native-speech --output work/carried-speech
```

The demo retains `shared-carrier/you-would.native`, `you-would.wav` and
`you-would.json`. It then starts another process to decode the retained Native
components, execute inventory/correspondence/structure admission again, replay
the Language commitment request, and check the original source and projection
bytes. The JSON retains the selected Plan, active Plays and Sign events.

The independently authored positive specimen is **“you would”**, with intended
phonemes separate from target phones `[j u w ʊ d]`, two syllables and declared
word/turn boundaries. It has exact phone durations, fundamental cycles and
relative amplitudes. The older “would you weigh” specimen remains structural
conformance; its standalone `[e]` has no binding in the compact renderer.

These are technical PCM-computation artifacts. They establish no device
playback, attended intelligibility, natural-speech quality or trained voice.

## Checked quantity composition

`SpeechPhoneAcousticScope` retains an exact phone occurrence, complete IPA
inventory basis (including Variety and inventory revision), original full text
reference and Audio anchor. Numeric-only helpers do not establish that custody.

`SpeechFundamentalBinding` separates its target from an optional supplied
observation. Preparation requires the original opaque correspondence and
executes Audio's reciprocal laws. Intended and observed frequencies never
replace each other. Source and generic sample-projection receipts retain
unreduced ratios and remainders on declared sample clocks.

`SpeechEnergyBinding` separates an amplitude or power trajectory and referenced
dB basis from an optional supplied observation. Error specifications are absolute
linear ratios relative to the same basis, preserving all six specification
states. They are not inferred dB uncertainties. Audio owns interpolation,
amplitude/power conversion and the finite exact-power-of-ten dB profile;
unsupported logarithms refuse. A declared reference is not calibration evidence.

`SpeechAcousticScope` adds actual admitted syllable, resolved source word,
positioned phrase boundary/phone interval and whole-utterance cases to the
original phone scope. Preparation retains the original correspondence and the
same opaque syllable structure. Matching an isolated label is insufficient.
`SpeechFrequencyGestureSet` distinguishes fundamental, formant-center and
bandwidth roles while generic Audio owns the Hz trajectories, all-original
coverage and overlap priority/refusal. A selected fundamental alone can project
to a cycle. `SpeechScopedEnergyGestureSet` applies the same scope admission to
amplitude/power controls with exact declared references. Competing controls
with one identity cannot change their role or reference under priority.

Supplied frequency observations have distinct measured and inferred cases,
original scope and anchor, query/span, generic Hz, confidence, provenance and
all six error-specification states. An exact allowance means zero error; a
positive allowance reuses Audio's positive Hz Type. Admission neither performs
nor authenticates a measurement. These finite sets do not assert arbitrary
additive coarticulation or support interpolation that Audio refuses.

`SpeechLinguisticRateTarget` declares a constant full-utterance span and the
actual admitted syllable count. The checked Source projects an unreduced exact
syllables-per-second ratio. It is not acoustic Hz or an inferred measurement.
This one-span profile does not admit a schedule, interpolate gaps or merge
overlapping rate spans.

## Contrasting terminal projections

The common canonical evidence identity includes original intent, phonemes,
phones, complete text, realization alignment, full inventory/notation basis and
executed notation matches. Separately supplied syllables, positioned boundaries
and word correspondence are bound by the enclosing retained artifact; the
correspondence identity alone does not claim to include them.

`FrozenCarriedFormant` freezes only explicitly admitted original phone choices,
timing and controls. It retains original canonical bytes and conversion
receipts. Its 8 kHz integer endpoints must agree with generic Audio cumulative
sample-projection receipts on the declared clock. The private evidence envelope
retains each exact Source program once and references it from every execution,
with the original input and output bytes; it preserves unreduced fractions. Unsupported phone features refuse. An opaque `PreparedOfflinePcm`
requires a checked declaration matching the computed original and projection
identities. The installed factory binds that exact declaration, source,
projection, sample clock and compiled DSP artifact into the selected Plan.
The kernel compares the input source identity, holds it through the final PCM
transaction, stages its cursor until atomic commit and stops publication after
cancellation. It never pronounces replacement text.

The offline declaration is caller-authored computation provenance. It is
neither authenticated human identity nor Language parser commitment or physical
playback authority. The conformance separately executes the existing Language
commitment Source on the same lexical source. Any later device playback still
needs its normal admitted Host authority.

The versioned model projector has one implementation shared by training-input
and inference-input preparation. Version 1 maps independent phoneme and phone
streams into checkpoint-private lexical token IDs. Exact full vocabulary and
inventory basis must match; matching labels alone cannot admit changed contents.
All original structure/acoustics remain on the original carrier. No acoustic
model Back is admitted by this lexical map: its terminal outcome is the explicit
`NoAdmittedAcousticBack` refusal. This establishes neither model training nor
ConduitVoice-to-FARGAN inference.
