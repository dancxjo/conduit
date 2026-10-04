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

`timing.conduit` adds exact rational seconds, strictly positive fundamental
cycle durations, and dimensionless relative amplitude. Duration and cycle
identity differ even for equal fractions; neither is a renderer frame count.
`intent.conduit` carries these quantities through the existing six specification
states, with separate phoneme/phone identity, exact occurrence/source references,
stress and provenance. Boundary/pause intent is a separate event. These are
speech intents, distinct from an execution Plan or an acoustic observation.

The optional preparation module `timing` projects admitted durations/cycles into
an explicit sample rate through checked `speech/time-at-rate` arithmetic. It
retains the original fraction, whole frames and exact remainder; it neither
normalizes the ratio nor chooses a rounding policy. Native output laws verify
the quotient/remainder relation. Product overflow refuses. For example, a
61/8000-second cycle gives 61 frames at 8 kHz and 122 at 16 kHz. A 1/3-second
duration at 8 kHz gives 2666 frames plus 2/3 of a frame. Independently dropping
per-segment remainders can accumulate drift; timeline commitment must account
for them. The private numeric carrier in `timing_projection.conduit` is not a
public admission path. Rust only projects admitted fields into that carrier
and reconstructs the law-checked native result.

`intent_prosody::prepare_segment_prosody` lowers known segment duration, cycle
and relative intensity through the existing checked projections. It retains
borrowed source intent, cumulative timing and exact control remainders before
pairing with an explicitly supplied ordered segment tape. All five unresolved
states refuse with the original specification and event index. Boundary events
refuse in this segment-only adapter; boundary intent remains a separate contract.
Rendering borrows fixed prepared storage. This does not establish source
resolution, phone selection, language commitment or a whole utterance admission.

`boundary_admission::prepare_boundary` uses an explicitly supplied native
`SpeechFormantBoundaryBinding` to realize known word, phrase or turn intent.
Native laws forbid mismatched kinds and unsupported compact relations; the
adapter retains the original source references/provenance and exact duration.
Every unresolved kind or duration state refuses with its original specification.
Known durations compose with `duration::prepare_duration_render` for exact silent
PCM spans. This is separate boundary preparation, not linguistic boundary
inference, uncertainty selection, commitment or whole-utterance admission.

This is the first shared-intent prerequisite for
[#4898](https://github.com/dancxjo/conduit/issues/4898) and
[#4907](https://github.com/dancxjo/conduit/issues/4907). The current compact
renderer consumes its existing event tape with optional exact-duration projection;
full rich-to-compact segment realization,
linguistic analysis, commitment and FARGAN execution remain unfinished. The
new intent does not claim that artifact references have been resolved, that
uncertain values have been committed, or that both voices already consume it.

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
snapshot's byte prefix against its actual UTF-8 text and checking feature-map
uniqueness are **additional admission work**, not established by a round trip
through these types. Existing Tongues runtime recognition/provider adapters
have not yet switched to these contracts.

The optional `admission` module checks ordered floating-second spans and finite
unit-interval scores explicitly marked as probabilities. These borrowed checks
preserve exact F64 bits and leave log/provider score domains unchanged. Callers
must invoke them during preparation, including after decoding: current native
Type laws do not support F64 ordering, so construction alone does not prove
these relationships. They do not establish artifact-reference validity or
complete rich-to-compact admission.

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
For pure voicing, the second and third resonators receive the difference
between the current and previous voiced-source values; the first receives the
full source. Continuous voiced-fricative frames separate their source roles as
described below; other noise-containing frames retain their mixed excitation. This
addresses higher-branch low-frequency energy filling spectral valleys, following
the parallel-model guidance in [Klatt (1980), section II.G](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1980-JAS000971.pdf).
One additional I32 history value retains the exact previous voiced source,
including across authored silence; it does not represent measured phonation or
change phone identity. Initial history is zero. The admitted pure source stays
within ±1600, so its difference stays within ±3200 and the existing ±4096
resonator-drive proof envelope. Portable frame checks cover the new state;
focused source tests cover steady-source rejection, exact noise bypass and
selected overflow refusal. `nasal-place.wav` compares m, n and ng before aa and
iy. This does not yet implement nasal pole/zero insertion or adjacent-vowel
nasalization, and subjective improvement remains unverified.

For v, dh, z and zh, the first resonator receives pure voicing while the upper
noise path receives pure turbulent noise. The authored selection requires voiced and
frication flags both equal to one and no closure; stops and affricates retain
their release policy. Four self-authored models use a 250 Hz, 140 Hz-bandwidth
low voicing bar and the same noise transfer as their unvoiced partners.
This follows the separate-source motivation in [Klatt (1980), section II.F](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1980-JAS000971.pdf),
using a compact low-band approximation rather than a full separate voiced
cascade. It adds no retained history or runtime allocation. Selected phone,
derivation, stress and duration remain exact. `voiced-frication.wav` compares
f/v, th/dh, s/z and sh/zh before aa. Independent source-isolation and decoded-pole
laws accompany portable frame parity; listening-quality improvement is unverified.

Diffuse f/v and th/dh noise uses a direct Q8 bypass with self-authored gains
64/256 and 48/256. Their upper resonator gains are zero; unvoiced partners also
have zero low-band drive, while voiced partners retain the low voicing bar.
For s/sh, the low noise drive is zero; z/zh retain their low voiced source and
share the corresponding upper noise bands. Other classes have zero bypass,
including closure/release profiles. The bypass is added before the existing
output gain, envelope and limiter. A zero bypass skips its arithmetic entirely.
It adds one I32 to the transient acoustic target, with no new retained history.
These transfer choices are informed by [Klatt (1980), section II.F](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1980-JAS000971.pdf),
without reproducing its full parallel bank, high-frequency range or tables.
Phone identity, duration and uncertainty remain unchanged. Source-role and
spectral-window tests accompany portable parity; general listening quality
remains unverified.

The raw modulo turbulence is scaled by one third before filtering, so its
mean-square excitation energy is below that of the glottal source throughout
the admitted pitch range. The authored mixer applies a fixed output gain of
16 before its explicit limiter. This balances the previously overpowering noise
and raises the quiet PCM level; it is a profile setting, not WAV normalization
or a loudness measurement. Listening-quality acceptance remains open.

The discrete flow difference sums to zero over each fixed-period cycle and
retains the closure impulse before its zero closed interval. Both the scalar
source query and the frame graph share the same authored normalization, flow
and noise-mixing policies through exact context-carrying cords. The cubic pulse itself adds no history or waveform table; upper-branch
differencing adds one fixed history value. No allocation is needed. These are profile defaults, not measured
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
boot, device drivers and rich preparation metadata. They exercise default
timing/pitch/intensity rather than explicit control tapes. The probes render directly
into caller-owned static PCM storage, observing each produced sample with a
volatile read and a small checksum; they avoid a second stack output buffer.
The proof retains each `_start` disassembly and reports its register-save and
fixed stack-subtraction bytes. It also retains complete linked disassembly with
the ELF symbol table and a `stack-inventory.json` report. That inventory separates
function entry reservations, direct call sites and identified computed-control
sites from embedded data. It does not sum entries into a call-chain bound:
computed targets, body stack adjustments and call-chain liveness are unverified.
Its machine-readable `full_stack_status` remains `unproven`, and unsupported
entry shapes retain an explicit gap instead of a numeric reservation. The current
default-profile renderer has five identified computed-control sites, and text plus synthesis has
47; these are linked-code inventory facts, not execution or device-fit proof. Unsupported prologue shapes refuse inspection.
This is an entry-stack lower bound: callee frames, later body stack changes,
boot and interrupts are excluded. It cannot establish total stack or device fit.
A link is not device playback or proof of real-time performance. WAV generation
writes hosted artifacts and does not access an audio device.

Remaining work includes artifact-reference validation and explicit rich-to-
compact segment admission, cardinal/ordinal/decimal/date normalization and broader pronunciation,
broader consonant-vowel transitions and vowel-dependent stop-burst spectra,
coarticulation and additional Klatt controls, a public CLI WAV-output route and device playback, and physical timing/footprint proof. The samples do
not establish Klatt/eSpeak parity or multilingual voice coverage.

Explicit exact durations enter through the optional preparation adapter
`duration::prepare_duration_render`. It retains every original duration ratio
and projects cumulative event ends with floor at the formant profile's fixed
8 kHz grid. Per-event counts are differences of those endpoints: three
one-third-second events produce 2,666, 2,667 and 2,667 frames, totaling exactly
8,000. Fractions remain in the timing receipts. Common-denominator arithmetic
uses checked U64 multiples; overflow refuses rather than approximating.

`Renderer::prepare_timed` also accepts already projected caller-owned frame
counts, with exactly one count per event. Plots admit the profile domain and
scale stop closure with the segment's duration; their acoustic coefficients
remain unchanged. Segments need at least two frames. Boundaries may have zero
frames and continue to separate neighboring segment contexts without adding
samples. The existing event/utterance/block bounds still apply. Preparation
refusal leaves caller grid storage unchanged. Rendering borrows fixed storage
and allocates nothing; rich timing receipts allocate only during preparation.
These receipts do not attest commitment, language references or provenance,
and unknown/unspecified/alternative durations are not silently selected.

The supported native-speech proof also retains `duration-default`,
`duration-faster` and `duration-slower` WAVs of the same “Hello, world!” event
sequence. The faster/slower listening fixtures explicitly author duration
factors of 2/3 and 3/2. Resolved pitch and output-relative intensity can be added before play as described below.

An explicit compact `VoiceEvent.selected` carries a `RealizationResult`: the
source phoneme/stress/position, selected phone, and derivation remain separate.
The formant plots consume that phone directly, including timed segments and
adjacent-segment models, instead of running the English allophone selector again.
Existing `segment` and `pronounced` inputs still run the authored selector.
Supplied selection is an input, not an attestation that its derivation or
external provenance was validated. Rich utterance/reference admission and
rich prosody-state selection remain unfinished.


Resolved native cycles and relative intensities enter through the optional
`control::prepare_voice_control` adapter. The source ratios and exact projection
remainders remain in checked native receipts. Cycles project to 1/256-frame
coordinates, and relative amplitude projects to 1/32768 precision. A requested
120 Hz cycle at 8 kHz yields 17,066 Q8 units with remainder 80/120 of one Q8
unit; retained subframe phase avoids rounding every cycle to 66 whole frames.
Plots admit periods of 8 through 512 frames and relative amplitudes of 0 through
2. Unsupported controls and checked arithmetic overflow refuse explicitly.

`Renderer::prepare_controlled` borrows one projected control per event.
`Renderer::with_controls` can add controls to a prepared duration renderer before
play; replacement after the first produced frame refuses. Duration, exact phone
selection, and cycle phase share one bounded cursor. Boundaries freeze the
fractional phase. Relative amplitude scales the complete realized PCM, including
voicing and frication, then saturates at the existing signed PCM limits. Zero
amplitude still advances the admitted speech; it is distinct from a boundary or
cancellation. Unity amplitude with explicit profile selection preserves the
existing waveform. The explicit-cycle source uses the existing cubic flow,
noise balance, and resonators; its open duration remains a declared whole-frame
profile approximation.

The supported proof retains profile-half, 120 Hz, 200 Hz, and 120 Hz half-amplitude
listening fixtures. These controls do not select unknown, unspecified,
not-applicable, or alternative prosody states, validate rich utterance references,
or expose rich controls through the public kernel input. Those integrations and
FARGAN conditioning remain unfinished.

`VoiceEvent.phone(SpeechPhoneInput { phone, stress })` renders an explicitly
supplied profile phone without inventing a source phoneme or word position.
Its `realization()` projection returns `None`; the complete event remains in
the prepared tape. The existing plot-authored target lookup, transitions,
trajectory, source, and filters also serve this path. Exact timing and voice
controls apply to it, with the same finite bounds and staged cursor behavior.
The supported proof retains `direct-phone-hello-world`, whose supplied-phone
PCM matches the ordinary phoneme frontend. This profile input is distinct from
rich `PhoneId`/inventory resolution and does not attest external provenance.

The direct-phone renderer and retained realizations share one lowered phone model
through the plot-authored `speech/selected-phone` coordinate projection. The
standalone Cortex-M0+ probes measured 13,587 bytes code/constants for rendering
and 25,332 bytes for text plus rendering, with 260/1,284 bytes BSS and no data.
This adds 72 bytes relative to the pre-direct-phone measurements. All 19 listening
WAVs remain byte-identical. These are default-profile link measurements with the
exclusions above, not full-stack, device playback or realtime acceptance.

Under `semantic-bindings`, `inventory_admission::resolve_inventory_phone` binds an
already resolved material phone token to a unique definition in a supplied
`SpeechInventory`. Native Conduit laws check the snapshot's inventory and language
against that inventory, and check the requested and selected definition identities.
The result borrows the original material, inventory and complete definition; it
preserves features, aliases, status, confidence, acoustic evidence and provenance.

Only a `known` phone ID permits lookup. Other specification states are returned
unchanged in `InventoryRefusal::Unresolved`; no default or gradient/variable
selection is implied. Missing and duplicate requested IDs are distinct refusals.
Matching IPA or alias spelling does not substitute for identity. This is local
snapshot/definition resolution, not registry authenticity, voice-profile eligibility,
commitment admission or rich-to-compact realization. It adds no compact DSP policy
or playback allocation.

`text_admission::resolve_text` binds a text source reference to a supplied
immutable `LanguageText` snapshot. Native `LanguageTextReferenceMatch` laws
check text identity, revision, language and scalar range extent. The preparation
helper derives the actual Unicode scalar count, converts scalar positions to
UTF-8 boundaries only for slicing, and retains the original reference and
material alongside its checked receipt. Empty spans are legal. Segment kind is
retained without claiming that the selected range is linguistically a word or
phrase. This local resolution does not authenticate a registry, establish
translation acceptance, or commit speech. Its bounded hosted preparation is
separate from compact MCU playback and adds no DSP policy.

Rich material phones enter the formant renderer through
`profile_admission::prepare_profile_phone` after exact material and inventory
resolution. The caller supplies a native `SpeechFormantVoiceProfile`: its bounded
bindings name complete phone definitions and typed compact phones. Conduit laws
require the compact profile's exact IPA spelling for each binding and exact
inventory/language membership. Lookup refuses missing or duplicate bindings and
any difference between the resolved and bound definition metadata. Opaque phone
IDs and aliases are never interpreted as IPA or compact enum names.

The preparation receipt borrows the original resolved material, inventory,
profile, binding and stress specification and names the checked DSP source. Its
compact event contains a supplied phone, with no invented source phoneme or word
position. Known stress, unknown and unspecified remain distinct. Not-applicable,
variable and gradient stress are returned as typed unsupported states. Feature
constraints on definitions or observed tokens also refuse explicitly until their
acoustic lowering is implemented; this profile does not silently discard them.
Canonical IPA spelling here is a finite eligibility rule, not an IPA parser,
feature-consistency proof, registry authentication or multilingual voice claim.

The supported proof writes `rich-phone-hello-world`. This declared listening
fixture keeps its rich sources and receipts alive through rendering and matches
the phoneme frontend PCM exactly. Complete utterance/source commitment admission
and planner ownership of rich preparation receipts remain separate work. Hosted
preparation may allocate within the native bounds; the compact rendering tape and
playback retain their existing finite-storage contracts.

Intent phone lookup also accepts an exact event in a `SpeechUtteranceIntent`
and a caller-supplied inventory. Native laws check occurrence membership and
inventory/language agreement; only a known opaque phone ID resolves to a unique
full definition. The preparation receipt borrows the original intent segment,
including its sources and provenance. It creates no observed token, confidence,
or material snapshot. Missing events, boundary events, unresolved specifications,
and missing or duplicate definitions remain distinct refusals. This lookup does
not yet select a voice profile or admit a whole utterance for playback.

`prepare_intent_profile_phone` pairs that resolved intent phone with an explicit
formant profile, retaining the original segment and profile binding. It uses the
segment's own stress specification and the same definition and feature checks
as material-token profile preparation. Unsupported stress retains its exact
specification; definition features are refused rather than discarded. This
prepares a phone event only: source resolution, phonological consistency,
quantitative prosody and utterance commitment remain separate admission work.
`prepare_utterance_timing` retains one original `SpeechUtteranceIntent` and
projects cumulative exact durations across both segments and boundaries.
An explicitly supplied native boundary profile contains at most three bindings;
missing and duplicate bindings refuse. Segment receipts retain their exact
cycle/intensity quantization; boundary receipts retain their original sources,
kind, binding and match law, with no invented cycle or intensity receipt. A
checked Plot supplies the ignored compact control carrier for boundary slots.

Its renderer checks event count, segment/boundary shape and exact compact
boundary realization before borrowing the frozen timing/control tapes. Phone
selection, occurrence/source resolution, phonological consistency and commitment
remain separate checks; this timing preparation is not complete utterance
admission. All unresolved quantitative values retain their original specification
and global event index, and all durations share one cumulative grid.
