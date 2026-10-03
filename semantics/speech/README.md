# Native speech and language segments

Development foundation for [#4832](https://github.com/dancxjo/conduit/issues/4832).
Conduitese owns the segment, listening, and translation meaning. Checked plots
own this compact English voice's realization choices and fixed-point equations.

- `types.conduit` adapts Speaking's phonetic/phonological inventories, distinct
  PhoneId and PhonemeId, specifications, features, environments, allophone rules,
  provenance, floating-second observations, and exact frame spans. Acoustic
  evidence keeps optional timing and full feature values. Unknown, unspecified,
  not-applicable, variable, and gradient remain different states.
- `listening.conduit` adapts recognition events, ASR stability candidates,
  decoding/session controls, word commitment, and analysis candidates. UTF-8
  stable-prefix byte lengths differ from Unicode-scalar revision ranges;
  occurred/observed clocks, calibrated/provider-native scores, hypotheses,
  commits, cancellation, completion, and end-of-stream remain distinct.
- `translation.conduit` relates exact text revisions and segment occurrences.
  Text, phoneme, phone, and recognition references differ. Alignments admit
  reordering and many-to-many correspondence; omission, insertion, and unresolved
  links are explicit. Proposal, acceptance, and withdrawal differ. Language and
  locale differ. These are structures for i18n, not a translation engine or
  locale formatter.

`pronunciation.conduit` owns this first English orthography profile's case
normalization, classification, small self-authored dictionary, ordered spelling
rules, word position and punctuation decisions. `pronounce` prepares ordinary
text into caller-owned storage, retaining its exact borrowed source and scalar
ranges. Dictionary choices differ from spelling guesses; unspecified stress
stays unspecified. Dictionary phonemes reference the whole source word; spelling
phonemes reference the consumed cluster. These are pronunciation correspondences,
not measured acoustic alignments or universal dictionary correctness claims.

The source is adapted from Tongues/Speaking revision
`b5d7535b2ccd3730b4db878fb61b2e4e48ae89c9`. Collection/text limits are this
profile's admission bounds. Values exceeding them refuse; they must not be
silently truncated. Provider attributes retain bounded JSON text (format validation belongs to its adapter),
without treating it as typed semantic truth. Native constructors enforce local
bounds and declared laws. Resolving references against retained artifacts,
checking text ranges against their exact revision, and checking a stability
snapshot's byte prefix against its actual UTF-8 text, ordering floating-second
spans, and checking feature-map uniqueness are **additional admission
work**, not established by a round trip through these types. Existing Tongues
runtime recognition/provider adapters have not yet switched to these contracts.

The optional `semantic-bindings` feature generates ordinary rich native Rust
bindings for preparation and inspection. It uses allocation and is separate
from the small renderer. The default rendering crate has no runtime dependencies
and uses no allocator. A narrow build-time lowering compiles eligible checked
portable expression trees and finite acyclic pure-expression graphs from
`voice.conduit` into integer Rust functions. Every wire resolves to an exact
typed value before compilation; missing inputs, cycles, duplicate writers,
non-value tracks and unsupported operations refuse the build. The generic
prepared expression Back currently refuses variant construction; this pack uses
its separately tested compiled Back, and does not claim generic kernel coverage
for those programs. No Rust phoneme table,
realization rule, or DSP equation substitutes for the plots.

This first voice accepts a closed English phoneme profile with explicit stress
and position. It retains the input and realization derivation with the selected
phone. It uses three parallel Q14 resonators at 8 kHz, bounded excitation/noise,
closure/envelope, and pauses. Private DSP values use signed 32-bit integers.
Conservative profile bounds cover any admitted clamped filter history, source
excitation, noise update, interpolation and mixing intermediates. This compact
arithmetic choice does not narrow the rich segment/listening evidence types. `trajectory.conduit` owns a finite stop-release
noise window and within-phone diphthong coefficient glides. The acoustic
endpoints model spectra; they do not split one diphthong into phoneme or phone
occurrences, infer stress, or claim measured alignment. Plain stops have a
96-frame release window; aspirated stops and affricates retain their profile's
frication after release. The self-authored reduced-bandwidth stop models
separate low/mid/high release energy, with a rapid release attack and the shared
trailing fade. Plain voiceless stops release 128 frames before their end;
voiced stops release earlier and retain a voiced tail. Aspirated stops retain
a longer noise tail. These are profile defaults: vowel-dependent burst spectra,
voicebars during closure and separate aspiration filtering remain unfinished.
The closure/release/voicing separation and rapid plosive onset are informed by
[Klatt's cascade/parallel synthesizer paper](https://sail.usc.edu/~lgoldste/Ling582/Week%2012/klatt1980.pdf);
this profile does not reproduce its full controls or published parameter tables.
Caller-owned traversal admits at most 256 events,
30 seconds, and 128 frames per advance. A candidate copy can be discarded under
output pressure before committing progress. This is synthesis Back conformance,
not an additional scheduler or execution kernel. `connection.conduit` blends adjacent continuous voiced models over at most
128 frames at each side of a join, using an exact shared spectral midpoint.
It removes the intervening envelope fades for those joins. The current phone's
excitation class, duration and identity remain unchanged; a sequence edge and
word/phrase/turn boundaries remain distinct and block the join. The adjacent
models come only from the retained event tape; the traversal never skips a
boundary to find a different neighbor. `onset.conduit` derives a stop-place
model from the exact selected preceding phone and moves the following vowel
from a reduced onset toward its existing temporal target over 160 frames.
Labial/alveolar endpoints differ; the velar endpoint brings the current vowel's
F2/F3 coefficients together. These self-authored coefficient-space models keep
the current phone, duration, excitation and stress, and never cross a boundary.
Differential proof covers all phones, stop places, window edges and arithmetic
refusals; finite-window pole checks establish resonator stability. Formant
transitions as a separate stop-place cue are informed by
[Story and Bunton's production/perception study](https://pmc.ncbi.nlm.nih.gov/articles/PMC3145491/);
these defaults do not reproduce its model or measured parameter data.
Separate aspiration and broader coarticulation remain unfinished. These are
modeled transitions, not measured acoustic alignment. `prosody.conduit` adds a bounded within-segment
pitch-period contour for known primary, secondary, unstressed and reduced stress.
The `speech/pitch` graph carries the actual stress, target, frame, excitation
history and envelope flags through base-period selection and contour calculation
via exact authored cords. Both pitch policies have one authored owner. The
`speech/prosodic-frame` graph connects that exact pitched frame to the nine-stage
DSP graph. The `speech/contextual-frame` graph also carries that context through
vowel-onset preparation and blending before prosody/DSP; Rust calls the complete
sixteen-stage onset/prosody/DSP graph. The `speech/connected-frame` graph retains
the exact selected phone, stress, history and neighbors through connection,
pairs its model/envelope result with that context, and feeds onset/prosody/DSP
through authored cords. The `speech/temporal-frame` graph also composes the
three temporal-target stages and their context projection before connection,
yielding one twenty-four-stage synthesis graph. The trajectory equations specialize
through existing Conduit type parameters for model-only endpoint requests and
real synthesis context; endpoint requests carry no invented frame history.
Non-segment neighbors retain their relation and receive the exact current
temporal model as their neutral endpoint. Rust calls this synthesis graph once.
The three-stage `speech/stress-intensity` graph precedes pitch. It carries the
exact context through level selection, a bounded edge contour and gain scaling.
Known secondary, unstressed and reduced sonorants have relative interior levels
224/256, 192/256 and 160/256; primary, unknown and unspecified remain neutral.
Stops and frication remain neutral. First/last 128-frame transitions return
sonorant gains to the exact neutral model at segment joins. Only the three gains
change; phone identity, stress, duration, spectral model, excitation flags and
history are retained. Neutral branches return the exact input without running
scaling arithmetic. These are self-authored profile cues, not measured loudness
or inferred syllable stress. Portable checks cover every phone/stress state,
context preservation and selected overflow refusal; independent laws cover gain
bounds, neutral joins and monotone edge contours. `stress-prosody.wav` presents
eh with primary, secondary, unstressed, reduced, unknown and unspecified stress
in that order. Subjective prosody and intelligibility remain unverified.
Unknown and unspecified stress retain the neutral period throughout; the contour
never changes the segment's stress specification. Portable differential checks
cover checked arithmetic refusals and all stress states; admitted contours stay
within 58–69 sample periods. These profile defaults do not model phrase-level
intonation or establish measured prosody. The voiced source in `glottal.conduit`
uses a self-authored Q8 cubic flow pulse with an open phase of
`floor(3*period/4)`, followed by an exact discrete
difference. This is informed by the source/flow model in
[Klatt and Klatt (1990), section II.B](https://www.source-code.biz/klattSyn/Klatt-1990.pdf),
without copying an implementation or their parameter tables. It is a limited
source model, with no spectral-tilt, aspiration, flutter or diplophonia controls.
The discrete flow difference sums to zero over each fixed-period cycle and
retains the closure impulse before its zero closed interval. Both the scalar
source query and the frame graph share the same authored normalization, flow
and noise-mixing policies through exact context-carrying cords. No new history,
waveform table or allocation is needed. These are profile defaults, not measured
phonation features or a new phone identity. Portable source-edge/refusal parity
and independent cycle laws accompany the frame proof; subjective quality is
unverified. The checked `speech/frame`
plot composes excitation, three resonator transitions and output mixing through
exact authored cords. Initial history and
boundary silence also come from checked plots. Rust traversal handles bounded
event/frame iteration and caller-owned output. The `speech/frame` graph owns
its DSP stage sequence. The four-stage `speech/segment-model` preparation graph
composes realization and voice-model lookup while retaining the exact original
input, selected phone and derivation alongside the acoustic target. Its generic
lookup shares one authored table with the scalar phone-model entrance. Portable
proof covers all 40 phonemes, six stress states and five positions. The
seven-stage `speech/neighbor-endpoint` graph selects the exact adjacent phone
model at its declared start or end, carries the selected stop-place cue
through the shared temporal equations, and returns both together. The scalar
cue query shares the same authored policy. Portable proof covers all 44 phones
at both ends, including arithmetic refusals. Rust currently connects segment
preparation, exact adjacent-tape projection, endpoint and temporal-frame calls. Consolidating that entire
chain into one authored graph needs explicit pairing of retained frame context
with called results. The current unary pure-value compiler does not invent
implicit synchronization or erase a Flow join's temporal contract.
The pack's lowering asks Rust to inline only small (at most 128 expression nodes)
context projections/selectors inside composed graphs. Numeric equations and
standalone helpers retain normal compiler sharing decisions. This is a code
layout hint; checked operations, refusals and identities remain exact.
Whole-frame differential proof covers every phone at envelope edges and compares
full-duration state/sample history against the checked scalar composition
with temporal targets. A separate portable-graph differential checks every
phone at closure, release and glide edges; acoustic checks cover nonzero plain
stop release, stationary monophthongs and preserved diphthong durations.
Independent spectral analysis of rendered plain-stop PCM checks the distinct
energy distributions; this is acoustic model evidence, not human intelligibility.
Sample rate and admission limits come from the checked literal
`speech/profile` plot, without a second Rust copy of that policy.

The optional `kernel` feature provides the exact `speech/english-utterance`
contract, offer and `NativeSpeechBack`. It accepts one bounded text value and
emits canonical `audio/pcm-frames@1` on a closing Flow: mono s16le at 8 kHz,
128-frame maximum blocks, a configured semantic media-clock identity, and
contiguous frame positions. Preparation checks the selected Fore, Kind,
implementation, source-bound artifact, limits and absence of resources/authority.
The kernel retains the original text value until the last block commits; private
segment ranges therefore remain source-scoped. The Back stages a copied cursor
and fixed PCM bytes, then advances only in `step_committed`. Existing kernel
fan-out owns atomic pressure, storage, cancellation and terminal signs.
Cooperative fuel reserves one bounded computation quantum before text/event/
frame traversal, plus two I/O actions; this is not instruction containment or a
CPU timing guarantee. Preparation may allocate descriptions; play does not.

The supported proof checks an ordinary planned graph lowered through
`conduit-plan-lowering`, with two exact PCM sinks. It establishes byte equality
with the standalone renderer, capacity-one fan-out pressure, cancellation versus
completion, malformed versus unsupported text, and zero heap allocations during
prepared play. The installed std Host reference composition includes this Back; a minimal
composition selects it with `with_native_speech`. `conduit check` recognizes
the voice Kind and `PcmFrames` output alias. Installed execution proof uses an
ordinary plot with exact bounded external text/PCM Fores and verifies all bytes
and terminal signs. Device playback and a CLI WAV-output route remain unfinished.

The text profile admits at most 512 UTF-8 bytes and 32 bytes per word, within
the same 256-event and 30-second rendering bounds. It handles ASCII English
letters, apostrophes, whitespace and the punctuation declared in the checked
classification plot. `normalization.conduit` reads ASCII digits as individual
English digit names, preserving leading zeroes. Its phoneme/stress rows agree
with the profile's word-name dictionary; they remain authored models. Each
emitted phoneme carries `digit_name` provenance and the exact original digit's
scalar range. The normalization plot owns the separators before and after each
digit name. Letter/digit mixtures remain source-scoped. Cardinal, ordinal,
decimal, date and locale interpretations are not inferred. Other unsupported
characters explicitly refuse with a scalar offset/codepoint. Preparation preflights event count and caller storage;
refusal leaves storage unchanged. Its fixed dictionary chooses profile defaults;
spelling guesses do not resolve all English irregularities, homographs or stress.
Artifact identity and rich-to-compact admission remain separate work.

Generate phoneme-authored and ordinary-text WAVs and run focused conformance:

```sh
cargo xtask prove journey native-speech
```

Add `--microcontroller` to link both standalone Cortex-M0+ footprint probes.
This needs the `thumbv6m-none-eabi` toolchain target, GNU `size`
and the pinned Rust `llvm-tools` component for section/entry inspection. The renderer probe includes constants and a 128-frame static output buffer.
The text probe additionally retains 64 typed events in fixed storage and includes
normalization, dictionary/rules and text traversal. Both exclude planner, kernel,
boot, device drivers and rich preparation metadata. The probes render directly
into caller-owned static PCM storage, observing each produced sample with a
volatile read and a small checksum; they avoid a second stack output buffer.
The proof retains each `_start` disassembly and reports its register-save and
fixed stack-subtraction bytes. Unsupported prologue shapes refuse inspection.
This is an entry-stack lower bound: callee frames, later body stack changes,
boot and interrupts are excluded. It cannot establish total stack or device fit.
A link is not device playback or proof of real-time performance. WAV generation
writes hosted artifacts and does not access an audio device.

Remaining work includes artifact-reference validation and explicit rich-to-
compact segment admission, cardinal/ordinal/decimal/date normalization and broader pronunciation,
broader consonant-vowel transitions and vowel-dependent stop-burst spectra,
coarticulation and additional Klatt controls, a public CLI WAV-output route and device playback, and physical timing/footprint proof. The samples do
not establish Klatt/eSpeak parity or multilingual voice coverage.
