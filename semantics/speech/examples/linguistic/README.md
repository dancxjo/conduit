# Committed linguistic carrier conformance (#5666)

This candidate proves one supplied three-word utterance, `would you weigh`,
through the existing checked Language commitment Source, ordinary Speech
Source construction, Native serialization and re-admission. A fresh process
inspects the same immutable artifact using its original content identity.
It re-executes commitment and inventory admission; a digest or decoded candidate
alone supplies neither membership nor permission to speak.

The executable owner is
[`committed_carrier`](../../tests/committed_carrier.rs). The supported repository
entrance is `cargo xtask check speech-ipa`.

The fixture supplies its pronunciation and variety explicitly. Its seven
intended phonemes and seven target phones include a two-to-one `/d j/` →
`[d͡ʒ]` group crossing the `would`/`you` source spans, and a one-to-two diphthong
→ vowel/offglide group in `weigh`. The underlying `contrast/j` occurrence and
realized `target/glide` occurrence belong to different groups. Phone spelling
never determines phoneme identity. Existing `TranslationCorrespondence`
records retain exact word-to-intended-phoneme links as bounded segment relations;
no translation candidate or second utterance IR is introduced.

The artifact contains the original Language text revision, lexical tape and
profile, intent, distinct phoneme/phone tapes, complete inventory and IPA basis,
explicit definition bindings and realization relation, syllable and boundary
components, all four accepted IPA constructor result Types, and the exact
commitment request/result and Source bytes. Inspection checks those components
against one another and re-encodes every Native value without changing bytes.
The Source specimen reconstructs the existing `SpeechUtteranceIntent` Type.

All six specification cases occur in original duration intent. The three
syllables retain onset/nucleus/coda, primary or unstressed lexical stress,
original Unicode IPA byte/scalar spans, exact word spans and variety. Eight
positioned events retain all seven boundary kinds. Missing, foreign and duplicate
segments, stale commitment, changed Source, foreign variety and excess component
counts refuse after decode. A changed lexical-stress value cannot masquerade as
the original artifact even when it remains a valid new Native value.

This is a bounded semantic and kernel fixture, not pronunciation inference or
linguistic-quality evaluation. The accepted parser profile permits at most four
word tokens; this specimen commits three. Existing limits remain 256 speech
tokens/events, 64 alignment groups, eight members per populated group, 32 phones
per syllable and one nucleus per syllable. The inspection evidence envelope
permits 20 components and at most 2,000,000 bytes; it contains existing canonical
Native packets and introduces no public transport format. Unsupported shared or
multi-nucleus syllables, unresolved alignment members, complete morphology,
speaker/style references and acoustic tracks remain outside this specimen.
Text finality, stability, semantic commitment, availability and playback remain
separate. No #5262 acoustic, #5263 contrasting-realization, neural, audio,
physical-playback or human-listening acceptance is claimed.

The fixture consumes accepted Language Source and its existing kernel harness.
No code is recovered from archived #5273. The upstream alignment carrier from #5925 is admitted at
`6a2910056f980cb329eb52aa969fc89f9b59dd6a`. This proof consumes that exact
source. Local tests do not establish accepted combined `dev` evidence.

## Explicit duration grid preparation

`intent_duration_grid::prepare_intent_duration_grid` composes the same native
`SpeechUtteranceIntent` with Audio's `PreparedAudioSampleProjection` append laws.
It borrows the original intent and complete text, checks source occurrences,
and accepts an explicit abstract elapsed-second anchor and optional sample-rate
basis. This subset accepts text-sourced intents against one complete immutable
text material; other source-material kinds refuse. The anchor must match; absent rate, stale text, unresolved duration,
foreign denominator and unsupported numeric bounds refuse without changing the
carrier. This preparation does not grant commitment or playback authority.

The committed carrier test includes a separately authored timed specimen: all
seven target segments explicitly last 1/3 second. It re-admits that new original
artifact before independently projecting it at 8, 16 and 48 kHz. Each cumulative
floor endpoint and remainder comes from executed Audio Source; three appends
reach exactly one second instead of accumulating independently rounded frames.
The original six-state specimen is retained and refuses at its unknown duration.

This bounded bridge requires one unchanged denominator across the tape, the
Audio projector's 32-bit source fraction bound and at most 192 kHz. Mixed
fractions require an explicit future common-grid admission rather than silent
normalization. The API uses the existing Speech duration carrier and retains
its exact numerator/denominator when constructing the generic Audio request;
it adds no independent quantity or sample-clock arithmetic. It does not add
F0, amplitude/power/dB, overlapping trajectories or neural realization.
