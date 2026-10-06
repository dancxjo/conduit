# Language: portable meaning and exact realization

Language is the shared semantic layer for linguistic content. Particular
languages are data and Back truth, not branches in Conduit's general model.
This contract is owned by #4955; identity consolidation is owned by #4956 and
realization coverage by #4957. The latter remains unfinished development work.

`semantics/language/identity.conduit` owns `LanguageId`, `VarietyId`, their
explicit `LanguageVariety` relationship, and `LanguageText` identity, revision,
value, scalar ranges and text occurrence references. Rust bindings derive from
these checked declarations. Language imports no speech contract. Speech,
listening and translation consume the same native types and Rust carriers;
translation owns correspondence, not the underlying text occurrence.

A Language identity is an opaque bounded domain identity. A variety's relation
to its language is explicit. It need not be regional or a standard tag: a
historical, reconstructed, experimental or session-local profile can have its
own identity. Language, variety, locale, script, voice, country and user
identity are distinct. Locale remains localization policy; choosing a language
never chooses a locale, script or voice.

External tags and provider-private names require an exact declared mapping.
Neither a BCP 47 prefix, ISO number, model row, array position nor a provider's
name establishes semantic identity or compatibility. Ambiguous or lossy mapping
requires the [projection report](projection.md); unknown identity refuses
without English substitution. Detection evidence names hypotheses; it does not
establish a Back's coverage or select a route.

Portable Kinds describe work such as tokenization and syntax without a language
name. Inventories, lexicons, morphology rules, word order, pronunciation and
prosody profiles belong to selected data or Backs. Universal Dependencies is a
portable interlingua, not a universal grammar or a promise of lossless mapping.
Unrepresented distinctions remain explicit through the projection boundary.

The current four-token/four-annotation reference is a bounded deterministic
specimen. Its tiny English annotation rules do not become the definition of
Language. A structurally different specimen may use the same types without
introducing another subsystem. Neither specimen establishes production parser
accuracy or general multilingual support.

The #4907 streaming spine must consume these identities and exact source-text
revisions rather than define parser-local language/text cousins. Analysis
revision and source-text revision remain distinct; the shared bounded revision
lifecycle does not make ASR confidence into coverage or commitment into effect
authority.

## Ownership migration

The previous speech-owned `SpeechLanguageId`, `SpeechVarietyId` and
`ListeningTextRange` names are replaced by Language-owned types. Text and
occurrence declarations move out of speech's translation source. Speech-only
inventory, utterance, phone/phoneme sequence, listening event and translation
alignment contracts remain with their owners. No permanent duplicate identity
or compatibility alias is introduced. Historical documentation and retained
proof keep the names they actually recorded.

Renaming nominal declarations changes their native semantic identities. This
requires fresh checking/planning rather than silently treating an old contract
as current. The tracked-source/data audit found no persisted fixture using the
retired names that requires compatibility decoding. Persisted compatibility,
if a current consumer requires it, must be
an explicit separately tested decoding boundary; it cannot be inferred from
similar record shapes.

Listening consumes an exact Language mapping projection before constructing its
language hypothesis, preserving the selected variety and independently supplied
confidence. Provider-private names remain in the projection report. This
conversion does not establish ASR accuracy, coverage or an execution route.
