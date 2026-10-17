# Common linguistic utterance carrier audit (#5664)

This is a field audit of accepted declarations, not whole-utterance admission.
Conduit baseline: `16e60c289` on `dev`. Tongues architectural source:
`b03702798db5a5e7d174278ac2fc1233f171b02e`, under
[`crates/speaking/src`](https://github.com/dancxjo/tongues/tree/b03702798db5a5e7d174278ac2fc1233f171b02e/crates/speaking/src).
The earlier adaptation pin is `b5d7535b2ccd3730b4db878fb61b2e4e48ae89c9`.
Use the architectural pin for the source fields below. The accepted canonical
IPA proof is [#5660](https://github.com/dancxjo/conduit/issues/5660), including
its exact published-source receipt. Names or serialized IDs alone do not
constitute membership, commitment, or permission to speak.

The owner remains [SpeechUtteranceIntent](../../semantics/speech/intent.conduit),
composed with existing Language, inventory, sequence and syllable components.
Do not replace it with a second utterance IR. “Preserved” below means a field
has a native carrier; the admission column distinguishes executed checks from
shape-only transport. “Unsupported” means a caller must retain the original
outside this slice and explicitly report loss if projecting into it. An absent
optional field never means an invented known value.

The inspected source blobs (Git SHA-1, at that exact commit) are:

| File | Blob |
|---|---|
| `utterance.rs` | `f8c458fdccb159f7050ad8b08062a552d0cb0d73` |
| `spec.rs` | `168bf3ed9d8ced0b568535960b50ab0e390686d1` |
| `phonology.rs` | `a7c87ed76dad014e3eaeb801c5a716618be0fd7c` |
| `phonetics.rs` | `840b740b505c8289df9465f386122e9f102cccad` |
| `prosody.rs` | `e37ea21be2eb4fd14232daea7286116ce8fcefe8` |
| `segment.rs` | `b48aa3a286676d992254d65dd5b480dc52ee9f90` |
| `morphology.rs` | `278f20156e113136ef7f898e456b740a2c26d1b1` |
| `acoustics.rs` | `48b8eb1cc1b49e032d7bf20671bbd6c333f9fdb1` |
| `evidence.rs` | `b67332af540a00a7b4babe1f169eb67baf6fca7c` |

## UtterancePlan fields (utterance.rs)

| Original field | Existing carrier / disposition | Admission and remaining obligation |
|---|---|---|
| `id` | Preserved: `SpeechUtteranceIntent.utterance_id` | `validate_intent_occurrences` checks event basis; it does not establish uniqueness or commitment. |
| `variety` | Preserved as component: `LanguageVariety`, `SpeechIpaInventoryNotationBasis.variety` | Intent itself carries Language, not Variety. Retain full checked IPA basis/profile and inventory; matching language names are insufficient. |
| `speaker` | Optional upstream; unsupported here | No speaker inferred from variety or selected voice. |
| `intended_text` | Optional upstream; preserved through `LanguageText` and exact `LanguageTextSegmentRef` sources | `resolve_intent_sources` retains original complete text. Intent has references, not an embedded text copy. |
| `intended_morphemes` | Partial: text refs distinguish morpheme and word spans | Lexical/morphological analyses and all upstream token attributes are unsupported by this intent; text spans alone are not those analyses. |
| `intended_phonemes` | Preserved component: `SpeechPhonemeSequence`, `SpeechPhonemeToken`; segment `phoneme` | Inventory admission checks supplied definitions; complete contextual occurrence correspondence is #5665. |
| `target_phones` | Preserved component: `SpeechPhoneSequence`, `SpeechPhoneToken`; segment `phone` | Universal IPA is notation. Explicit phone definition bindings and inventory checks remain necessary. |
| `target_syllables` | Preserved component: `SpeechPlannedSyllableIntent`, `SpeechSyllableSource` | `PreparedSyllableStructure` checks source, roles, nucleus, duplicate membership and order. No syllabifier is implied. |
| `boundaries` | Preserved structural kinds: separate intent events and `SpeechPositionedBoundary` | All seven kinds are transported. Positioned preparation checks exact basis, range and source order. Compact realization supports only explicitly bound word/phrase/turn. |
| `target_prosody` | Partial: stress, boundary duration and per-segment prosody; existing trajectory components | Full pitch/energy/rate curves, timed break confidence and label track are not carried as a complete upstream ProsodyTrack. Unit integration belongs to #5262. |
| `target_acoustics` | Unsupported complete AcousticFrame track | Never substitute observed evidence or renderer frames. #5262 owns acoustic integration. |
| `speaker_reference` | Optional upstream; unsupported | Reference audio URI and embedding/description remain omitted explicitly. |
| `style` | Optional upstream; unsupported complete StyleRef | Existing careful-style evidence is not a reference-audio/embedding style carrier. |
| `provenance` | Preserved: `SpeechEvidenceProvenance` | Source/method/optional version; provenance is a claim, not authority. |

`Utterance` is the separate observed representation: `id`, `variety`, `speaker`,
`text`, `morphemes`, `phonemes`, `phones`, `syllables`, `boundaries`, `prosody`,
`provenance` follow the same field dispositions, but observed values must not be
relabeled intent. Its extra `graphemes` and `acoustic_frames` have no complete
counterpart in this intent slice. They are declared unsupported projection
losses, not empty observations.

## Component fields and custody

| Pinned source | Target and field disposition | Checks / limits |
|---|---|---|
| `spec.rs::Spec<T>` | All six cases preserved by `SpeechSpec<T>` in `types.conduit` | Known has T; Unknown is missing knowledge; Unspecified imposes no supplied choice; NotApplicable is distinct absence; Variable retains ordered alternatives (1–4); Gradient retains T and finite confidence 0–1. No implicit default or resolution. Upstream empty or >4 Variable refuses this profile. |
| `phonetics.rs::Phone` | `id→identity`, `ipa`, `features`, `aliases`, `status` preserved by `SpeechPhone` | ID/IPA ≤64 bytes, ≤16 features, ≤4 aliases. Text nonemptiness alone is not IPA admission; `PreparedIpaInventory` executes parsing and definition binding. |
| `phonology.rs::Phoneme` | `id→identity`, `notation`, `features`, `default_phone`, `possible_phones`, `aliases`, `allophones`, `status` preserved by `SpeechPhoneme` | ≤8 possible phones and allophones. A default is explicit data, never inferred from a realized phone's IPA. |
| `PhonemeAllophone` | `phone`, `environment`, `conditions`, `confidence`, `status`, `source_rule_id→source_rule_identity` preserved | ≤8 conditions; native environment and rule preparation retain unresolved evidence. This is not a complete pronunciation engine. |
| `PhoneToken` | `phone`, optional `span`, `features`, `acoustic_evidence`, `confidence`, `provenance` preserved | ≤8 acoustic observations. Seconds and frames are distinct span variants. Seconds ordering requires local preparation. |
| `PhonemeToken` | `phoneme`, optional `span`, `features`, `realized_as`, `confidence`, `provenance` preserved | ≤4 nested realized phones. Nested copies do not establish many-to-one occurrence identity; #5665 must check original tapes. |
| `prosody.rs::Syllable` | `phones→occurrence refs`, `stress`, `phone_positions`, optional `span`, optional `nucleus_index` preserved | 1–32 members, equal role count, nucleus role/index laws. Structure preparation requires exactly one nucleus and disallows shared membership; weaker component construction alone does not. |
| `morphology.rs::MorphemeToken` | `surface` and optional `span` can be retained as original text plus resolved span; `morpheme`, `features`, `pronunciation`, `confidence` are unsupported as a complete token in this intent | Word/morpheme segmentation does not recreate lexical identity or feature assertions. |
| `prosody.rs::Stress` | Primary, Secondary, Unstressed, Reduced preserved | Lexical stress remains a specification on the syllable, distinct from notation marks and per-segment stress. No stress inferred from pitch. |
| `segment.rs::SpeechBoundaryToken` | Kind retained; source and position restored by explicit text reference and phone gap | `after_grapheme_index` is not a phone-gap index: explicit correspondence is required. Optional text `span` maps only when supplied and resolved; `terminal` punctuation and `pause` classifications are unsupported by these boundary records. Intent adds separate duration/provenance. |
| `evidence.rs::EvidenceProvenance` | `source`, `method`, optional `version` preserved | Nonempty method ≤64 bytes; version ≤64 bytes. No source priority ladder or authorization copied. |
| `acoustics.rs::AcousticObservation` | Cue identity, optional span, specification, confidence preserved by `SpeechAcousticObservation` | Generic feature value is not an admitted unit-bearing acoustic target. |

Text custody uses [Language identity](../../semantics/language/identity.conduit):
text ID, exact revision, language, original ≤4096-byte UTF-8 text and half-open Unicode-scalar ranges. IPA separately retains original
spelling, byte/scalar/unit spans and profile/basis. Neither cursor unit is a
phone count: tie bars and combining diacritics form declared multicodepoint
units. No normalization may silently replace original bytes.

Speech occurrence custody uses `LanguageSpeechTokenRef`: utterance, sequence,
revision, inventory, language and ordinal. Phone/phoneme references are distinct
`LanguageSegmentRef` cases. Full sequence material is ≤256 tokens. Inventory
identity alone has no revision field: retain the exact complete inventory and
its checked notation basis (including variety, revision and full profile).
Re-admission must execute those checks again; Native decode is not an opaque
`PreparedIpaInventory` receipt.

Intent bounds are ≤256 events and 1–8 sources per event. Structure preparation
accepts 1–64 syllables and ≤256 positioned boundaries; their positions denote
gaps 0..=phone-count, including the final gap. Different boundary kinds can
coincide in supplied order. This finite profile does not admit ambisyllabicity,
multi-nucleus syllables, unlimited alternatives or universal phonological
analysis. These cases refuse rather than being flattened.

The unsupported `ProsodyTrack` fields are explicitly `pitch.points`,
`energy.points`, `speaking_rate.points` (each point: `time_s`, `value`,
`confidence`), `breaks` (`after_s`, `duration_s`, `boundary`, `confidence`)
and `labels` (`span`, `kind`, `confidence`) as a complete coordinated track.
Existing segment quantities are partial components, not a lossless conversion.
The omitted complete `AcousticFrame` fields are `span`, `f0_hz`, `energy_db`,
`voicing_probability`, `periodicity`, `harmonicity`, `formants` (index, Hz,
bandwidth Hz), `spectral_centroid_hz`, `spectral_tilt_db_per_octave`,
`zero_crossing_rate`, and `vectors`. Their units and target/observation
distinction belong to #5262; no dimensionless placeholder is substituted.

## Proof and next slices

`specification_custody` exercises all six existing native cases through encoded
bytes and structured re-admission, preserving alternative order and exact
confidence; empty/excess alternatives and invalid confidence refuse.
Existing `semantic`, `ipa_membership`, `intent_sources`, `occurrence_membership`,
`material_references`, `syllable_intent` and `linguistic_prosody` tests own nominal
identity, original material, membership and structural refusals. Run the
supported repository entrance `cargo xtask check speech-ipa`; focused Cargo
invocations are internal development checks. At this audit checkpoint, 32 tests
passed across those eight focused targets (including the new specification
target); this is local candidate proof, not accepted whole-carrier integration.

C1 adds no linguistic Type. #5665 must add checked occurrence correspondence
without guessing phoneme identity from realized phone spelling. #5666 must
retain one original committed carrier, serialise and independently re-admit
its full materials and commitment; a decoded candidate or matching text is not
commitment. Existing revision/commitment ownership stays with #4907.

Archived #5273 (`6806f08fbb07723237441b97c93cebf5d1e72138`) was inspected for
scope: its constructor work is already accepted; renderer/playback owners do
not fill a C1 gap. No code is recovered here and the omnibus PR stays closed.
This audit establishes neither #5262 acoustic integration nor #5263 contrasting
realizations, model compatibility, audio production, or human listening proof.
