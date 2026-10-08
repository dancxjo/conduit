# Supported IPA notation and exact source preservation

The notation entrance extends Speech with a supplied, versioned notation profile.
It does not identify phonemes or phones from strings, authorize playback, select a
language or variety, or turn an uncommitted parser hypothesis into speech intent.
The common utterance aggregate and terminal renderer integration remain separate
unfinished parts of #5212 and #4907.

`SpeechIpaNotationProfile` retains the complete exact inventory identity,
LanguageVariety, revision, unit definitions, explicit spelling aliases and their
provenance. `SpeechIpaUnitId` identifies a notation unit within that supplied
profile. It is deliberately neither PhoneId nor PhonemeId. Unit kind separates
segments, primary/secondary stress, length and syllable boundary. A segment may
contain several Unicode scalars: `t͡ʃ`, `pʰ` and `n̩` each remain one declared unit.

Version1 supports a deliberately finite subset of IPA unit syntax. The exact
accepted base symbols and multiscalar spellings are executable in
`semantics/speech/src/ipa_unicode.rs`. This is a notation syntax subset, not an
English phoneme enumeration or a claim that other IPA combinations are invalid.
An unsupported spelling refuses. Extending the subset requires a reviewed,
versioned grammar change and corresponding tests; arbitrary nonempty strings are
not accepted as IPA definitions. Definitions using `ih`, `ax`, `ch`, or
`p_aspirated` belong to separately named provider/renderer imports, not this
notation entrance. No ASCII lookup infers vowel length, rhoticity or realization.

The notation reference is the [International Phonetic Association's official
chart](https://www.internationalphoneticassociation.org/IPAcharts/IPA_charts_EI/IPA_charts_EI.html).
The current subset supports the issue's tie-bar, aspiration, nasalization,
syllabicity, stress and length examples. Supporting these spellings does not
establish an actual language's inventory or allophone rules.

No automatic Unicode normalization occurs. Both `ã` and `ã` are supported
spellings. They remain separate definitions unless an explicit authored alias
maps one spelling to the other's unit. Version1 permits only that declared alias
pair. Aliases never discard original spelling or provenance. A profile cannot
contain duplicate unit identities or duplicate spellings, including aliases.

A phonemic display requires `/…/`; a phonetic display requires `[…]`. Delimiters
remain in the original text but are excluded from unit spans. The partitioner
counts complete supported partitions and refuses ambiguity, even if a greedy
longest match would succeed. Each occurrence retains a half-open UTF-8 byte span
and exact source spelling. Byte offsets never become phone ordinals. Unrecognized
suffixes refuse the whole input; no partial success or truncation is returned.
Stress must have a following segment. Length must immediately follow a segment.
A syllable boundary must follow a segment or length and cannot end a transcription.
These syntax checks do not infer a syllable nucleus, lexical stress, acoustic
duration or the contextual realization of a phoneme.

The source contracts limit original input to4096 bytes, unit spellings to64 bytes,
and parsed occurrences to256. Profiles contain at most256 definitions and256
aliases. Preparation and parsing currently use ordinary allocating Rust paths;
these finite semantic limits are not a complete Flow allocation/work admission.
The same original spelling, full profile and checked profile-correlation record
must survive Native serialization. Raw profile-correlation values alone do not
prove that the parser executed or grant any downstream authority.

The original source commit records conformance with independent helper tests for exact multicodepoint
partitioning, explicit alias source preservation, ambiguity, duplicate spelling,
unsupported suffix and occurrence bounds. Its six recorded workspace tests passed across
`ipa_notation`, `ipa_membership` and `ipa_source_execution` with
`--features semantic-bindings`. They exercise Native round trips, full-profile
custody, foreign inventory/variety/revision refusal, delimiter/stress/length
refusals, and the actual fixed Source unit-syntax program. Every selected source
hash was unchanged across that gate. These are notation and admission results;
they do not establish inventory-definition binding, a common utterance aggregate,
terminal projection, bounded Flow execution, or committed multiword audio through
this notation entrance. Those remain acceptance work.

## Binding inventory definitions

`PreparedIpaInventory` binds the complete caller-supplied `SpeechInventory` to a
prepared notation profile. Explicit phone and phoneme bindings name their own
identities and the ordered notation-unit identities; equal spelling never makes
a phone a phoneme or supplies an allophone rule. Every definition must have one
binding, every binding must be consumed, and the parser's complete output must
match that binding. The original inventory, aliases, features and status remain
retained together with the executed phonetic or phonemic transcription for each
definition. Provider spellings such as `ax` cannot enter the `ipa` field through
this admission path.

`SpeechIpaInventoryNotationBasis` checks the inventory/language, complete supplied
LanguageVariety and notation-profile revision. This revision is the notation
profile's declared basis; the existing SpeechInventory and LanguageVariety Types
do not acquire a registry or independent revision authority here. A caller must
retain the actual complete admitted inventory and profile rather than substitute
objects sharing their IDs. The receipt establishes notation binding, not a
phoneme-to-phone realization, current registry membership, utterance commitment,
Flow resource admission or permission to play.

The second original source commit records eight passing workspace conformance tests across the four IPA targets, including
`ipa_inventory`. The additional cases exercise distinct phone/phoneme identities,
whole-value Native round trips, foreign inventory/language/variety/profile
revision refusal, missing bindings, wrong unit identities and provider-code
refusal. Its scoped Clippy across all four targets passed with warnings denied
and the `semantic-bindings,kernel` features. Shared utterance aggregation and
terminal projection remain unfinished.

## Qualified quoted Source authoring (#5260)

The inventory-independent phonetic profile and transcription have no Language,
Variety or inventory fields. A single admitted phonetic phone preserves existing
`PhoneSpecification.unknown`; a spelling does not mint a PhoneId. Inventory-bound
phonemic transcription is a different Native type. Its preparation checks the
complete supplied inventory, exact declared variety/revision, definition bindings
and unique partition into actual PhonemeIds.

The examples in `semantics/speech/examples/phonetic_ipa.conduit` and
`phonemic_ipa.conduit` call `speech/phonetic-from-ipa` and
`speech/phonemic-from-ipa` with ordinary quoted Unicode text. These are normal
Kinds offered by the std native-speech composition, with typed required startup
arguments, a selected Back and an ordinary HostCall returning a Native admission
outcome. They do not require proposed `p[…]`/`p/…/` shorthand or reinterpret
collections/patterns. The phonemic constructor receives its whole inventory as a
separate typed startup argument, preserving the existing Type depth limit.

Preparation validates the complete retained arguments and computes the finite
partition/outcome. Runtime correlates the exact placement and a SHA-256 digest
including all canonical startup arguments and placement/Host/boot identity before
publishing the prepared Native outcome. The request/outcome canonical ceilings
are 262144 bytes; original text is at most 4096 UTF-8 bytes, profiles at most 256
units/256 explicit aliases, and admitted transcriptions at most 256 occurrences.
One prepared constructor uses one admitted resource unit; the std image offers
16. Preparation allocates within this finite profile; these limits are not a
whole-process heap measurement or a claim that runtime parsing is allocation free.

Located refusals use half-open byte and Unicode-scalar offsets relative to the
original decoded IPA Text. The checked Source/Plan separately retains the owning
constructor's authored Source span. No Unicode normalization, greedy ambiguity
resolution, trimming or provider-symbol approximation is performed. Original
spelling, exact occurrence byte extents, unit definitions and explicit alias
provenance survive Native serialization. Scalar locations are counted from the
original text, never treated as segment ordinals. The safe
`phonetic_unit_source_span` view provides both byte and scalar extents for
serialized accepted occurrences and refuses invalid extents/spelling.

The existing lossless CST round trip, quoted-text serialization and syntax
highlighter cover this Unicode subset. No public Source pretty-printer/formatter
exists in the current Language/Plot API; formatting is an explicit unsupported
tooling boundary. Rust formatting does not prove Unicode Source formatting.

## Public notation and terminal projection audit

The authored examples above use actual IPA (`t͡ʃ`, `d͡ʒ`, `tʰ`, `n̩`, `ã`, stress
and length). Existing native inventory fixtures use real IPA such as `t` and `ə`.
Provider spellings `ax`, `ih` and `ch` in the IPA tests are negative refusal
fixtures. They do not enter the admitted public notation.

The legacy `EnglishPhone`/`EnglishPhoneme` variant alphabet in
`profile_phones.conduit`, `voice.conduit`, pronunciation dictionaries and compact
renderer controls is a terminal projection alphabet, not alternate public IPA or
universal phone/phoneme identity. The named `SpeechFormantVoiceProfile` binding
and compiled `SOURCE_ID` identify that terminal projection/version. Existing
`prepare_profile_phone`/projection-report admission retains the exact original
inventory definition and explicit supported-subset/refusal evidence; ASCII IDs
never substitute for the public `SpeechPhone.ipa`/`SpeechPhoneme.notation` fields.
No renderer/checkpoint code migration or new pronunciation/model projection is
claimed by this notation change.

Source provenance: the original notation/partition/Unicode/admission and three
proof modules are extracted from `4ae59f6985b2248fceb2dccf0d903aa1b0b03771`;
the complete inventory binding and its proof are from
`282862adab3bfc0ab115fa0b45e0278b44e05ba5`. They are reconciled against exact
foundation `d932da60c7f04bde62c797126e298e38aa9819d0`; other #5273 speech,
acoustic, gesture and renderer work is excluded. Original proof names
`ipa_notation`, `ipa_membership`, `ipa_source_execution` and `ipa_inventory`
remain intact. Added constructor journeys will be claimed only after their
actual gates pass.
