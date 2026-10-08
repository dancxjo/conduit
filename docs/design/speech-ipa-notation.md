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

Conformance includes independent helper tests for exact multicodepoint
partitioning, explicit alias source preservation, ambiguity, duplicate spelling,
unsupported suffix and occurrence bounds. Six actual workspace tests pass across
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

Eight workspace conformance tests pass across the four IPA targets, including
`ipa_inventory`. The additional cases exercise distinct phone/phoneme identities,
whole-value Native round trips, foreign inventory/language/variety/profile
revision refusal, missing bindings, wrong unit identities and provider-code
refusal. Scoped Clippy across all four targets also passes with warnings denied
and the `semantic-bindings,kernel` features. Shared utterance aggregation and
terminal projection remain unfinished.
