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

Language owns the legacy syntactic/discourse link vocabulary in
`semantics/language/syntax.conduit`. Speech rule conditions consume that exact
native Type through the checked catalog and external Rust bindings. The ordinary
language startup catalog exposes it to authored Plots;
`SpeechSyntacticLinkKind` remains a Rust re-export for callers. The optional
semantic-bindings profile carries this dependency, while the compact renderer
keeps its existing runtime dependencies. This ownership seam under #4907 does
not establish a complete dependency inventory, parser, revision basis, or
commitment frontier.

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

Scalar environment requirements now compare through the checked
`speech/context-compare` Plot. Borrowed receipts retain original stress, word,
syllable and prosodic specifications, including every uncertainty state. Only an
`Unspecified` requirement is unconstrained; a known requirement needs a known,
equal observation. Mismatch, unresolved requirement and unresolved observation
remain distinct. This also checks prosodic context explicitly rather than
reproducing the pinned upstream realization engine's omission of that field.
These comparisons do not infer observations or establish neighbor, feature,
style, syntax, source, commitment or complete allophone eligibility.

`compare_allophone_scalar_context` binds those comparisons to the original
allophone environment and four explicitly supplied observation specifications.
Its immutable receipt retains the declaration, including neighbor alternatives,
conditions, status and confidence. It reports each scalar decision separately;
it does not collapse unresolved evidence into a mismatch or claim eligibility.
The adapter allocates no storage and infers no occurrence or neighboring context.

`compare_allophone_neighbors` compares the original before and after pattern
lists against explicitly supplied immediate-neighbor observations. Each list
contains at most four alternatives for one neighbor, with four fixed receipt
slots. Empty lists are unconstrained. Native phone, phoneme and boundary laws
check exact identities; the neighbor Plot owns presence, domain and uncertainty
policy, and its alternative Plot owns disjunction. A match wins; otherwise
unsupported evidence precedes unresolved requirements, then unresolved
observations and mismatch.
Every original comparison remains available, including unresolved alternatives.
Absent neighbors and missing observation are distinct. Feature matchers use
explicitly supplied bundles, retaining every component receipt. These receipts
establish neither occurrence adjacency nor rule conditions, source resolution,
authority or complete allophone eligibility.

`compare_feature_bundle` validates unique keys on both supplied bundles and
retains at most 16 immutable fixed-slot receipts. Lookup preserves exact keys
without normalization; a native law verifies each selected identity. Missing
observations are explicit and preserve the original requirement. Native value
equality covers boolean, category, F64, bounded F32 vector and text values;
IEEE equality preserves encoded bits, including signed zero and NaN payloads.
The scalar Plot handles all six specification states, and conjunction runs in
Plot while retaining every component decision. Empty requirements are
unconstrained. Neighbor feature observations distinguish missing evidence from
a supplied empty bundle. No definition inheritance or feature inference occurs.
Native contract checks run during preparation; this does not admit allocating
structured equality during prepared play or establish a complete device budget.

`resolve_declared_intent_realization` checks that the chosen opaque phone ID
has at least one declaration in the exact requested phoneme definition of the
supplied inventory. It retains every matching default, possible-phone entry and
allophone declaration, including original indices, conditions, environments,
status and confidence. Native laws check the distinct phoneme and phone IDs;
missing/duplicate definitions, unresolved specifications and undeclared phones
remain distinct refusals with the original event index. There is no spelling or
IPA inference and no declaration ranking. Declaration membership is necessary
but does not establish contextual eligibility or choose an allophone; feature,
neighbor, style and linguistic conditions still need explicit resolution before
complete phonological admission. Preparation is bounded by at most 17 matching
declarations and performs no playback.

The supported proof writes `rich-phone-hello-world`. This declared listening
fixture keeps its rich sources and receipts alive through rendering and matches
the phoneme frontend PCM exactly. Complete utterance/source commitment admission
and planner ownership of rich preparation receipts remain separate work. Hosted
preparation may allocate within the native bounds; the compact rendering tape and
playback retain their existing finite-storage contracts.

`resolve_intent_sources` checks complete source coverage of one original native
intent. Callers explicitly supply one immutable text, phone, phoneme or
recognition-envelope material per source in event/source order. Native laws
check text identity/revision/language/range, speech sequence basis/ordinal, and
recognition stream/event identity. Receipts retain the original references and
borrowed materials; mixed source languages and revisions remain intact.
Missing or excess materials, wrong material kinds and failed native matches
refuse with exact event/source locations. At most 2048 receipts are prepared
from the native 256-event/eight-source bounds; the aggregate native structured
value node and canonical encoding ceilings also apply and can refuse an intent
before those collection maxima are reached. This establishes reference resolution
only, not source authenticity, causality, authority, phonological
consistency or commitment, and performs no playback or ambient lookup.

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

`prepare_intent_realization` composes chosen-phone lookup, explicit profile
realization and ordered timing from one immutable native intent. It owns the
compact event tape and retains original segment, inventory, profile and timing
receipts. Its renderer accepts no replacement event tape. Preparation validates
renderer domains and total frame bounds before returning, without playing PCM;
all collection growth occurs during preparation within the native bounds.
This remains chosen-phone quantitative preparation, not complete utterance
admission: source resolution, phonological consistency and commitment are still
separate checks. The `intent-realization-hello-world` listening fixture uses
explicit 120 Hz/unity controls and retains the approved acoustic profile.

Standalone rule input patterns have native phone/phoneme identity laws and
`speech/identity-pattern-compare`. `compare_phoneme_pattern` and
`compare_phone_pattern` retain all six original specification states and their
same-domain identity witness; only Unspecified is unconstrained. Known IDs use
exact UTF-8 identity, without notation normalization or base-ID fallback.
`compare_allophone_rule_input` retains the original rule, phoneme pattern and
explicit observed feature bundle, combining decisions through the native
context conjunction without dropping components after a mismatch. Missing
features remain missing observations; definition features are not substituted.
This prepares the input side of Tongues/Speaking-style standalone rules, not
occurrence binding, context completion, output-feature inheritance, status
permission, rule selection or linguistic commitment.
`compare_allophone_rule_context` binds an original standalone rule to an
exact intent occurrence and immediate event neighbors. Scalar requirements,
before/after alternatives and condition receipts retain the original rule
without constructing a temporary per-phoneme declaration. Matcher and stress
views share the same neighbors; real boundaries are retained and unobserved
endpoints remain unknown. Seven component decisions combine through the native
context conjunction without dropping mismatch evidence. Unsupported syntax and
occurrence failures remain typed refusals. Input matching, rule status policy,
output-feature inheritance, selection and commitment remain separate.

`evaluate_allophone_rule` combines those input and context receipts for the
same original intent segment. Optional feature observations retain their
provenance and must satisfy native equality across the occurrence's inventory,
language, revision, sequence, utterance and ordinal. Foreign occurrences produce
typed refusals; absent features preserve observation uncertainty. Both component
receipts remain available after mismatch. Evaluation does not select a rule,
resolve its output features or establish linguistic commitment.

`select_global_allophone_rule` prepares at most eight ordered standalone rules
from an explicit inventory/language profile. It checks the original occurrence,
profile basis and supplied feature occurrence even for an empty rule list.
Native status policy and choice laws select the first permitted match; an earlier
unresolved requirement or observation blocks lower matches. A Known requested
phone remains an exact constraint on the rule output. Every permitted candidate
retains its input, context and phone-constraint receipts, including candidates
after a winner; unsupported obligations refuse the profile with the rule index.
Excluded statuses retain an explicit unevaluated candidate. Candidate storage is
reserved once during preparation. This stage has no default candidate and does
not resolve output features, bind a voice profile, render or commit a phone.

`prepare_rule_output_features` retains an original standalone rule and explicit
optional default features and phone definition. The native
`speech/output-feature-layer` law selects default realization, phone definition,
then rule output by exact feature key. Later layers replace whole specifications,
including Unknown and Unspecified; only a Known output phone inherits lower
layers. Supplied phone definitions must match that Known phone's exact ID.
Missing layers stay missing. The receipt borrows at most sixteen resulting
features with their source layers; duplicate keys or an oversized merged result
refuse before returning output. This establishes feature inheritance over
supplied inputs, not inventory membership, occurrence provenance, rule selection,
output-phone resolution or commitment.

`prepare_chosen_global_rule_profile` binds a selected standalone rule's Known
output phone to one exact inventory definition and the admitted voice's complete
definition snapshot. Default-realization feature evidence is explicit, retains
its provenance, and must name the original occurrence. Native output-feature
inheritance retains the three layers before compact profile admission; every
nonempty inherited feature bundle is currently refused rather than discarded.
Unchosen/deferred states and selected unresolved outputs remain distinct typed
refusals. Timing, source resolution and commitment remain separate obligations.
The `global-rule-initial-aspiration-tata` fixture combines this bridge with native
quantitative timing from the original manual intent through the existing renderer.

`prepare_global_intent` combines global-rule choice, occurrence-bound default
features, exact voice admission and quantitative timing for one immutable intent.
Its evidence slots must align with the original events: segment evidence is
required and boundaries have no segment evidence. All segment obligations and
renderer admission must pass before a renderable result escapes. The private
aggregate owns its frozen event tape and retains each choice, inherited feature
receipt, definition/basis witnesses and profile binding. A late refusal returns
no playable aggregate; boundary silence and original uncertainty remain explicit.
Rendering borrows the admitted tapes. The
`global-intent-initial-aspiration-tata` fixture exercises this whole preparation
path. Source-material coverage and linguistic commitment remain separate checks;
the optional rich preparation receipts do not establish whole-device MCU fit.

`resolve_intent_inventory_phoneme` retains the original occurrence and one exact
phoneme definition from the supplied inventory. Native occurrence, inventory
basis and phoneme-identity witnesses are retained; a matching notation is not
membership. Missing or ambiguous definitions and all five non-Known states
refuse without case, Unicode or base-ID fallback. Whole global-intent preparation
now requires this receipt before rule choice and retains it alongside the chosen
phone. A late missing phoneme cannot produce a renderable aggregate even if a
wildcard rule and output-phone binding would otherwise match. Definition features
remain definition facts, without becoming observed token features or defaults.

`finish_global_default_choice` completes standalone-rule selection using only
the exact original phoneme definition's declared default phone. The existing
native finish law preserves winners and earlier deferral; default selection
requires explicit policy permission and compatibility with any Known requested
phone. The receipt retains the original choice, native phoneme membership and
optional exact default-ID witness. It does not invent a phone from spelling,
resolve phone membership, admit acoustics or establish commitment.

`prepare_global_default_profile` admits an already selected phoneme default
through the same inventory's unique exact phone definition and the voice's full
definition snapshot. Explicit default features require the original occurrence
witness; nonempty default or definition features remain typed unsupported input.
The receipt retains the selection, definition, feature provenance and exact
binding. It does not rewrite the intent or establish timing, source coverage or
commitment. The supported sample writer includes a declared-default “tata”.

Whole-intent global preparation now completes native default choice before voice
admission. Each private phone receipt retains the original rule-choice state,
completed state and an explicit rule/default realization. Rule output features
are available only for rule realization; default features retain their explicit
occurrence evidence. Mixed rule/default events and boundaries share the same
frozen timing and renderer. Earlier deferral or a late default-admission failure
prevents a playable aggregate escaping. This does not establish source coverage
or linguistic commitment.

`prepare_sourced_global_intent` resolves every original segment and boundary
source against explicitly supplied immutable materials in exact event/source
order before global rule/default realization. Its private aggregate retains both source
coverage and the prepared realization of the same original intent. Missing,
stale, foreign or wrong-kind materials refuse through the existing native source
contracts; failed realization cannot return a playable aggregate. The renderer
uses the frozen admitted events. Source resolution establishes reference
coverage, not authenticity, causality, authority or linguistic commitment.

Original allophone conditions have borrowed preparation receipts for explicit
careful style, previous/next matchers, singleton/set stress, and exact feature
keys/values. Their decisions are native Plots; the Rust adapters project typed
facts and retain the original requirements and observations. A condition-set
comparison admits at most eight conditions, keeps every successful component,
and uses the native context conjunction without short-circuiting mismatches.
Unsupported syntax is an indexed refusal, never implicit permission or a
mismatch. These checks do not establish occurrence adjacency, consistency across
independently supplied evidence, syntax revision/commitment, inheritance, rule
selection, or complete allophone eligibility.

Planned occurrence context retains the exact intent and current segment, plus
its immediate event neighbors. Segment neighbors require native occurrence
membership and `SpeechOccurrenceAdjacency`: equal utterance, sequence, revision,
inventory and language, with consecutive U32 ordinals. Duplicate occurrence
identities, skipped/reversed ordinals and foreign bases are refused. Boundary
events are retained directly and are never skipped to find another phone. An
unobserved endpoint remains unknown because the intent has no closure claim.

`compare_declared_allophone_context` binds one original compatible allophone
receipt to this occurrence context. Its scalar, neighbor and condition receipts
share that exact intent; a native conjunction retains their aggregate decision.
The index addresses the compatible declaration receipts, while the original
allophone ordinal remains on `PhoneDeclaration::Allophone`. Explicit syllable,
prosodic and careful-style facts remain required inputs; definition features do
not become observations. Matching does not change status/confidence, select a
rule, resolve inheritance, admit syntax, or advance commitment. This is hosted
preparation evidence; the compact renderer's acoustic plots are unchanged.

`select_intent_allophone` prepares an ordered choice from at most eight original
per-phoneme declarations. Native Plots apply the explicit five-status policy,
phone compatibility, priority fold and default permission. The first eligible
match wins; a higher-priority unresolved requirement or observation defers the
choice rather than falling through. Known phone requests remain constraints;
only Unspecified requests permit an unconstrained choice. All candidate receipts
retain their original declarations and exact occurrence context. Unsupported
context in an eligible declaration refuses this profile, including declarations
after a winner; excluded declarations are retained without evaluating context.

`prepare_chosen_allophone_profile` resolves the selected original phone to one
exact inventory definition and admitted formant binding without rewriting the
intent. This is hosted preparation with at most eight candidate receipts allocated before play, not complete
utterance admission or a whole MCU footprint claim. Standalone global rules,
inheritance, syntax, timing/source admission and commitment remain separate.
The native-speech journey emits `allophone-default-tata.wav` and
`allophone-initial-aspiration-tata.wav` from manually supplied linguistic facts
and the existing bounded renderer; acoustic parameters are unchanged.

`prepare_contextual_intent` combines contextual choice, exact profile binding
and quantitative timing into a frozen bounded event tape. Explicit context has
one slot per original event (Some for segments, None for boundaries). Deferred
or absent phone choices, incompatible profiles, unresolved quantitative facts
and renderer-domain failures refuse preparation before playback. Original
intent, choice and timing receipts remain inspectable; source resolution and
linguistic commitment are still separate obligations. Rich candidate storage is
reserved during hosted preparation within the native eight-rule bound rather
than copied as a large inline stack value. Rendering adds no collection growth.

`prepare_sourced_contextual_intent` requires complete ordered source coverage
against explicitly supplied immutable text, phone, phoneme or recognition
materials before preparing the contextual tape. Both receipts retain the same
original intent; stale revisions, missing coverage and realization failures
remain distinct typed refusals. No material search or revision substitution
occurs. Source resolution proves reference identity and range, not causality,
authority, segmentation or linguistic commitment. The sourced-contextual
listening fixtures retain the exact manual text revision alongside choice and
timing receipts.

The native choice transition carrier is a sum with 26 meaningful states: none,
default, and eight indices each for selection, requirement deferral and
observation deferral. Its shape prevents contradictory deferral reasons and
meaningless none/default indices. The public preparation receipt preserves
outcome/index/reason accessors and validates their correlation through native
laws. Pure choice Plots construct the sum; native preparation validates the
refined receipt separately, preserving the admitted law-validator boundary.

The sourced aggregate also admits mixed rule/default intents through the same
frozen event tape. The native-speech journey writes
`sourced-global-rule-and-default-tata.wav`: its initial /t/ uses the selected
aspiration rule and the remaining segments use exact declared phoneme defaults.
Source coverage, default policy, original occurrence evidence and voice admission
all finish before a renderer is returned; source coverage still does not imply
linguistic commitment.

The compact text profile has native regular-ending pronunciation for twelve
explicitly declared stems. Exact whole-word dictionary entries retain priority.
The checked inflection Plots identify bounded suffix candidates, declare stem
eligibility and select `-s` /s, z, ɪz/ or `-ed` /t, d, ɪd/ from the stem's final
phoneme. Rust only traverses those results and borrows prefix slices; it contains
no suffix spelling or voicing policy. Stem stress and dictionary provenance are
retained; appended segments have `inflection_rule` origin and exact modeled
suffix source spans. This is pronunciation correspondence, not a grammatical
analysis, lemma claim, measured alignment or linguistic commitment.

The declared `-s` stems are voice, world, device, conduit, synthesizer, speech,
listen, thank, please, want, speak and read. Regular `-ed` is declared only for
listen, thank, please, voice and want; the profile now includes a self-authored
American /wɑnt/ dictionary entry for want. At most two prefix candidates are
examined; each dictionary result has at most twelve phonemes and the ending has
at most two. Existing text/word/event bounds and atomic output admission apply.
Undeclared stems, consonant doubling, y changes and irregular past forms keep the
existing spelling fallback and its uncertainty rather than gaining inflection
provenance. This remains a small pronunciation profile, not general English
morphology or eSpeak-equivalent lexical coverage.

The pronunciation models follow the [British Council's -s teaching model](https://africa.teachingenglish.org.uk/classroom/pronunciation/snake-or-fly)
and [Iowa State's -ed/-s pronunciation chapter](https://iastate.pressbooks.pub/teachingpronunciation/chapter/chapter-5-ed-and-s-endings/).
The want entry follows the [Cambridge American pronunciation](https://dictionary.cambridge.org/dictionary/english/want).
The native-speech xtask proof writes `text-regular-plurals.wav` and
`text-regular-past.wav` through the same fixed-storage renderer; linked MCU
footprint and physical playback remain distinct proof classes.
