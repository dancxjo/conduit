# Shared speech acoustic custody

This development slice composes the existing `SpeechUtteranceIntent`, original
phoneme/phone sequences, checked IPA inventory basis, text correspondence and
syllable structure. It introduces no second linguistic IR. The field audit in
[speech-utterance-carrier](../audits/speech-utterance-carrier.md) remains the
complete Tongues mapping, including explicit omissions.

## Execute the conformance

```sh
cargo +stable xtask check speech-ipa
cargo +stable xtask prove journey native-speech --output work/carried-speech
```

The [retained evidence](../../site/evidence/shared-speech-carrier/manifest.json)
contains the [WAV](../../site/evidence/shared-speech-carrier/you-would.wav),
[Native packet](../../site/evidence/shared-speech-carrier/you-would.native) and
[Plan/Play/Sign report](../../site/evidence/shared-speech-carrier/you-would.json).
The supported suite independently re-admits that packet, replays the installed
formant path and compares its exact PCM and execution trace with those files.
The manifest pins byte lengths and SHA-256 digests; its producer code revision
is distinct from the later accepted Integration/publication revision.

The website stages these same bytes on the [public evidence page](https://dancxjo.github.io/conduit/evidence/shared-speech-carrier/).
Staging checks the manifest hashes, sizes, report bindings and WAV geometry. Its
`publication.json` records the staged source separately from the retained
producer; it is not an acceptance receipt. Native semantic re-admission and
PCM/Plan/Sign replay remain the Rust conformance test's responsibility, and the
normal publisher receipt establishes the accepted publication.

The journey selects IPA authoring, shared terminal and installed execution
checks rather than rerunning every Speech unit test. The separate
`xtask check speech-ipa` entrance and final exact-head CI retain broad domain
conformance.

The demo retains fresh `shared-carrier/you-would.native`, `you-would.wav` and
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
dB basis from an optional supplied observation. Measured and inferred supplied
interpretations remain distinct. Error specifications are absolute
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
amplitude/power controls and supplied measured/inferred observations with exact
declared references, spans, confidence, provenance and all six error states.
Competing controls
with one identity cannot change their role or reference under priority.

Supplied frequency observations have distinct measured and inferred cases,
original scope and anchor, query/span, generic Hz, confidence, provenance and
all six error-specification states. An exact allowance means zero error; a
positive allowance reuses Audio's positive Hz Type. Admission neither performs
nor authenticates a measurement. These finite sets do not assert arbitrary
additive coarticulation or support interpolation that Audio refuses.

`SpeechLinguisticRateTarget` declares a constant full-utterance span and the
actual admitted syllable count. Preparation borrows the original correspondence
and the exact same admitted syllable structure, checks the complete IPA
inventory/Variety basis, full source reference and Audio anchor, and retains
the original unreduced duration and provenance. The checked Source projects an unreduced exact
syllables-per-second ratio. It is not acoustic Hz or an inferred measurement.
This one-span profile does not admit a schedule, interpolate gaps or merge
overlapping rate spans.

## Contrasting terminal projections

`PreparedCompleteSpeechCarrier` composes the admitted correspondence, exact
syllable structure and positioned boundaries, lexical source and word relations,
actual Language commitment request/result, and explicitly supplied acoustic
receipts. Admission checks their shared owners and executes the existing
Language commitment Source; decoded booleans do not establish commitment.
Its canonical identity binds every supplied component and explicit absent
acoustic slot. Commitment evidence includes the owner Source identity, exact
program digests and input/output frames; the opaque Language receipt retains
the full programs without repeating their schema-rich bytes in each packet. Both model entrances and formant preparation require this same
opaque receipt. The correspondence digest remains a separately named subset
identity used when recovering original inventory bindings.

Version 1 admits one to four complete word tokens, matching the existing finite
Language commitment profile, with fully covered word-only lexical tapes and one source word per
word-to-phoneme relation, aligned phoneme-to-phone groups, and exactly one
syllable membership per phone. Other correspondence shapes or punctuation-token
carriers refuse explicitly at this entrance; the broader correspondence owner
continues to preserve inserted, deleted and many-to-many relations. Receipt
composition requires the same admitted owners, including pointer identity for
structure and acoustic receipts; equal separately admitted copies cannot be
mixed into an existing context.

The retained artifact reconstructs the complete receipt before either terminal
projection. Its outer file digest protects transport integrity; the complete
source identity binds the consumed input. A different valid structure therefore
cannot reuse an older model terminal or offline PCM declaration.

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
