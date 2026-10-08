# Shared speech intent: pinned Tongues mapping

This is an implementation audit for #5212 and a prerequisite ledger for #4907,
not a claim of completed shared IR. Tongues reference:
[`b03702798db5a5e7d174278ac2fc1233f171b02e`](https://github.com/dancxjo/tongues/tree/b03702798db5a5e7d174278ac2fc1233f171b02e/crates/speaking/src).
The initial audit used `45e6631050259d9a4b67759a0ca802bf392a22d9`; the implementation checkpoint below includes the published IPA/shared-owner and contextual-gesture integration.
Extend `SpeechUtteranceIntent` and Language contracts; do not introduce another
utterance architecture. A target is intent; an observed utterance is evidence.

Every field of Tongues `UtterancePlan` is listed below. “Partial” means the
current representation preserves some meaning but cannot support a lossless
aggregate mapping. “Missing” requires implementation or an explicit supported
profile refusal. No missing field may be silently reconstructed from PCM or IPA.

| Tongues field | Current Conduit meaning | Status and required work |
| --- | --- | --- |
| `id` | `SpeechUtteranceIntent.utterance_id` | Preserved; retain exact revision separately. |
| `variety` | `SpeechUtteranceIntentContext.variety` plus executed IPA inventory basis | Preserved by the prepared context and IPA/shared-owner join with exact Language/Variety/revision/profile. |
| `speaker` | Six-state external speaker reference in context | Preserved as retained metadata; no voice, reference-audio or embedding realization capability. |
| `intended_text` | Exact optional text reference plus borrowed original LanguageText | Prepared context checks scalar extent and exact revision; downstream committed projection remains required. |
| `intended_morphemes` | Planned morpheme components with original source texts | Prepared scalar-span, pronunciation and occurrence membership; conversion and downstream consumption remain required. |
| `intended_phonemes` | Original independent SpeechPhonemeSequence | Prepared shared owner retains the full original sequence and correspondence receipts. |
| `target_phones` | Original independent SpeechPhoneSequence | Prepared shared/IPA owners retain all candidates, exact inventory definitions and explicit many-to-many correspondence, insertion, omission and unresolved status. |
| `target_syllables` | Planned syllable component | Prepared ordered phone occurrence membership, positions, stress, optional nucleus and span; downstream conversion remains required. |
| `boundaries` | Typed boundary events with sources and exact duration | Partial; kinds cover phone/syllable/morpheme/word/phrase/breath-group/turn, but aggregate anchors and overlapping spans need admission. |
| `target_prosody` | Unit-bearing timing/pitch, linguistic-rate and intensity component ports | Components implemented with typed trajectories and exact elapsed-second anchors; complete aggregate owner, labels/break mapping and committed renderer projection still require proof. |
| `target_acoustics` | Separate formant/voice-quality targets and observed acoustic evidence | Typed center/BW, voicing, periodicity, tilt and observations implemented; joined-owner workspace tests pass. Harmonicity/vectors retain explicit unsupported interpretation profiles. |
| `speaker_reference` | Exact external identity and six-state specification | Retained-metadata-only profile explicitly refuses claimed audio/embedding support. Resource-bearing projection remains unsupported. |
| `style` | Exact external identity and six-state specification | Retained metadata and provenance; no inferred or copied realized style capability. |
| `provenance` | `SpeechEvidenceProvenance` | Partial; retain source/method/version and correlate exact Language derivation and original committed carrier. |

## Nested meaning and executed-conversion obligations

- `PhoneToken` and `PhonemeToken` already preserve separate identity
  specifications, optional spans, feature bundles, confidence and provenance;
  `PhonemeToken.realized_as` is bounded. The prepared shared owner now retains explicit
  many-to-many correspondence, insertion/omission/unresolved status; the IPA join
  separately checks inventory/variety membership without choosing candidates.
- Tongues `Syllable` has phones, stress, phone positions, optional span and optional
  nucleus index. The prepared syllable component now preserves that grouping.
  References must point to admitted occurrences rather than duplicate phones.
- Tongues `ProsodyTrack` has pitch, energy and speaking-rate curves, breaks and
  labels. Each curve point has time, value and confidence. Conduit must declare
  seconds/timebase and Hz, relative amplitude/power/dB reference, or linguistic
  rate; a generic numeric curve cannot supply these meanings by its field name.
- Tongues `AcousticFrame` has span, F0, energy, voicing probability, periodicity,
  harmonicity, formants, spectral centroid, spectral tilt, zero-crossing rate and
  acoustic vectors. Each formant has index, center Hz and bandwidth Hz. These now have ordinary typed component ports and evidence receipts; the
  prepared joined acoustic owner now passes actual workspace custody tests.
  Generic `SpeechFeatureValue` observations are not a unit guarantee.
  Harmonicity/vectors retain original material and unsupported interpretation profiles.
- Tongues speaker/style reference variants include reference audio and embeddings;
  style also includes manual/inferred variants. Preserve exact resource/profile
  references and provenance when supported. An absent optional value must not be
  reported as a copied speaker/style realization.
- Preserve all six `SpeechSpec` states. Unknown, unspecified, not-applicable,
  variable and gradient must not become zero-valued known quantities.
- Exact rational duration and cumulative remainder machinery already exist.
  Executed 8/16/48 kHz projections must retain the same original timing and pitch
  carrier while recording quantization and unsupported features.

## IPA and projection boundary

`SpeechPhone.ipa` and `SpeechPhoneme.notation` currently accept nonempty bounded
Text. That raw Native constraint is not IPA validation. The prepared notation and
inventory entrance now executes the finite declared Unicode IPA profile and
retains the whole inventory, explicit phone/phoneme bindings, original spelling
and complete parsing receipts; see [speech-ipa-notation.md](speech-ipa-notation.md).
Raw construction alone does not establish that receipt. The actual prepared IPA/shared utterance join now consumes this admission and
preserves every supplied candidate. Terminal projections still need to consume
the same joined owner.
Public data must use actual Unicode IPA and
explicit supported parsing/normalization conventions, preserving original
spelling and provenance. Multicodepoint affricates, diacritics, aspiration,
syllabicity, nasalization, stress and length need conformance proof. Delimiters
`/…/` and `[…]` are presentation, not inventory identity.

Compact English codes and FARGAN feature rows remain named terminal profiles.
Do not infer vowel length, rhoticity or phone identity by replacing ASCII labels.
A terminal projector must retain the original admitted carrier, selected profile,
executed conversion, projected values and fidelity/refusal report. Overlapping
trajectories are allowed; sequential phone events do not assert nonoverlapping
physical articulation. #5218 owns contextual phone-to-gesture realization.

The existing complete committed greeting formant proof retains both original
parser commitments. It does not yet establish this mapping's missing fields,
canonical IPA, paired neural realization, or physical playback. #5212 remains
open until implementation and conformance are verified on accepted `dev`.

## Current proof boundary

The actual fifteen-target workspace gate passed 39 tests with warnings-denied
Clippy. The subsequent contextual gesture/probability integration gate passed
16 tests with all 534 selected inputs unchanged. It includes the actual existing
opaque allophone choice: stressed onset `t` selects `tʰ`, unstressed onset selects
`t`, and the original phoneme/utterance stays intact. This does not bind that
choice to a separately prepared IPA/shared owner yet.

Audio step evaluation now has a separate U32 temporal cross-product profile and
copies full U64 quantities; the original bounded linear profile is unchanged.
Full-frequency values can execute at sample-time fractions without lowering
frequencies or changing canonical quantity limits. Actual acoustic-owner
composition passes three workspace tests covering all five ports, candidate
retention, original phone/text/event custody, scope/anchor refusals and actual
full-Hz sample-time evaluation. All 536 selected inputs stayed unchanged.
Warnings-denied Clippy and original-commit translation replay are running.

Gesture construction remains separate from renderer execution. Exact rate
projection, coefficients, closure/release/aspiration PCM contrasts, joined
committed multiword formant/FARGAN projections and attended listening remain
acceptance work. The original greeting WAV is not evidence for those additions.
