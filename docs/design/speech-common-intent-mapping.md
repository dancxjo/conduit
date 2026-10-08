# Shared speech intent: pinned Tongues mapping

This is an implementation audit for #5212 and a prerequisite ledger for #4907,
not a claim of completed shared IR. Tongues reference:
[`b03702798db5a5e7d174278ac2fc1233f171b02e`](https://github.com/dancxjo/tongues/tree/b03702798db5a5e7d174278ac2fc1233f171b02e/crates/speaking/src).
Conduit source inspected at `45e6631050259d9a4b67759a0ca802bf392a22d9`.
Extend `SpeechUtteranceIntent` and Language contracts; do not introduce another
utterance architecture. A target is intent; an observed utterance is evidence.

Every field of Tongues `UtterancePlan` is listed below. “Partial” means the
current representation preserves some meaning but cannot support a lossless
aggregate mapping. “Missing” requires implementation or an explicit supported
profile refusal. No missing field may be silently reconstructed from PCM or IPA.

| Tongues field | Current Conduit meaning | Status and required work |
| --- | --- | --- |
| `id` | `SpeechUtteranceIntent.utterance_id` | Preserved; retain exact revision separately. |
| `variety` | Intent has `language`; inventory is language scoped | Missing exact variety identity/revision and membership. Language alone is insufficient. |
| `speaker` | No aggregate speaker field | Missing; optional speaker reference must explicitly retain absence. |
| `intended_text` | Segment/boundary sources are `LanguageSegmentRef` | Partial; retain exact text/revision custody rather than add an unrelated string. |
| `intended_morphemes` | Sources may reference Language segments | Partial; missing aggregate morpheme/word correspondence and membership. |
| `intended_phonemes` | Segment `phoneme`; separate `SpeechPhonemeToken` contract | Partial; intended sequence and correspondence must survive independently of phones. |
| `target_phones` | Segment `phone`; separate `SpeechPhoneToken` contract | Partial; target sequence, insertions/deletions and many-to-many realization need aggregate custody. |
| `target_syllables` | Stress specification and syllable-position enum exist | Partial; explicit syllable component now retains identities, ordered phone membership/positions, optional nucleus and span with the original intent and phone sequence. Aggregate coverage and conversion remain unfinished. |
| `boundaries` | Typed boundary events with sources and exact duration | Partial; kinds cover phone/syllable/morpheme/word/phrase/breath-group/turn, but aggregate anchors and overlapping spans need admission. |
| `target_prosody` | Per-segment exact duration, fundamental cycle and relative intensity; Language prosodic intent | Partial; missing shared pitch/intensity/rate trajectories and labels with explicit timebase/domain. |
| `target_acoustics` | Generic acoustic observations and renderer-private controls | Missing unit-bearing shared acoustic targets, separately typed from observations. |
| `speaker_reference` | No aggregate field | Missing; supported reference-audio/embedding profiles need exact resource identity and bounded storage; otherwise explicit optional limitation. |
| `style` | No aggregate field | Missing; manual/inferred/reference styles need provenance and exact profile, or explicit optional limitation. |
| `provenance` | `SpeechEvidenceProvenance` | Partial; retain source/method/version and correlate exact Language derivation and original committed carrier. |

## Nested meaning and executed-conversion obligations

- `PhoneToken` and `PhonemeToken` already preserve separate identity
  specifications, optional spans, feature bundles, confidence and provenance;
  `PhonemeToken.realized_as` is bounded. This does not provide aggregate
  many-to-many alignment, inserted/deleted occurrences, or variety membership.
- Tongues `Syllable` has phones, stress, phone positions, optional span and optional
  nucleus index. Current position/stress enums do not preserve that grouping.
  References must point to admitted occurrences rather than duplicate phones.
- Tongues `ProsodyTrack` has pitch, energy and speaking-rate curves, breaks and
  labels. Each curve point has time, value and confidence. Conduit must declare
  seconds/timebase and Hz, relative amplitude/power/dB reference, or linguistic
  rate; a generic numeric curve cannot supply these meanings by its field name.
- Tongues `AcousticFrame` has span, F0, energy, voicing probability, periodicity,
  harmonicity, formants, spectral centroid, spectral tilt, zero-crossing rate and
  acoustic vectors. Each formant has index, center Hz and bandwidth Hz. These are
  missing from the shared aggregate. Generic `SpeechFeatureValue` observations
  are not a unit guarantee. Profile-specific vectors remain terminal projections.
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
Raw construction alone does not establish that receipt. The common utterance
owner and terminal projections still need to consume this admission.
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
