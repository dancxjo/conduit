# Language: portable meaning and exact realization

Language is the shared semantic layer for linguistic content. Particular
languages are data and Back truth, not branches in Conduit's general model.
This contract is owned by #4955; identity consolidation is owned by #4956 and
realization coverage by #4957. These are development contracts pending complete
proof and stable acceptance.

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
Language. A supplied Japanese specimen uses the same source, token, feature
and Universal Dependencies carriers for non-Latin scalar spans, a particle,
and different dependency ordering, without another subsystem. Neither specimen establishes production parser
accuracy or general multilingual support.

The bounded #4907 dependency revision consumer takes canonical `LanguageText`.
`TextSpan` and `LinguisticTokenIdentity` retain its exact `LanguageTextId` and
`LanguageTextRevisionId`; token and annotation bundles carry the immutable
material. Source identity, source revision, scalar range, surface mismatch, and
Language mismatch remain distinct refusals. A byte-based span converts through
the actual UTF-8 material and refuses a split scalar. Analysis revision and
source-text revision remain distinct; the shared bounded revision
lifecycle does not make ASR confidence into coverage or commitment into effect
authority.

## Incremental source revisions (#4907, development)

`text_revision.conduit` carries immutable finite `LanguageText` material, an
exact prior revision and sequence, optional Unicode-scalar stable prefix,
explicit partial/final status and linguistic provenance. The admission helper
checks consecutive sequences, unchanged identity/Language, prefix preservation
and a supplied finite revisable scalar tail. Finality describes completion of the
current snapshot, independently of revision closure: an edited authored source
may revise a final snapshot while preserving stable and committed material.

Stability, finality and consumer commitment remain distinct. A final source may
have no known stable prefix, and finality does not advance a committed frontier.
A candidate is prepared without mutating the prior source: publication and
consumer commitment must occur atomically at their owning execution boundary.
The helper alone proves neither kernel pressure/cancellation behavior nor ASR
ingestion, streaming parsing, speech playback or linguistic accuracy.

The prepared ASR bridge consumes the original finite recognition envelope and
an explicitly supplied Language/text/segment basis. Partial snapshots and
Unicode-scalar replacements produce the same native text revision family;
byte-based stability snapshots require exact segment/text correlation before
scalar conversion. Recognition finality, cancellation and consumer commitment
remain distinct. Stream, segment, source, Language and snapshot mismatches
refuse without publishing material. The borrowed original envelope retains
confidence, clocks and provenance. This bridge does not establish recognition
accuracy or a kernel-wired parser/playback route.

The development lexical profile retains up to four supplied alternatives per
whole reconstruction unit, with scalar spans and explicit correspondence to a
previous stable occurrence. Unknown units keep an empty candidate set; a CJK
alphanumeric run is not a claim of language-specific word segmentation.

The development symbolic parser profile has four token slots, one artificial
root and a finite stack. Checked source predicates own Shift, Reduce, LeftArc
and RightArc admission and mutation; native admission validates exact source
and analysis identities, graph laws and canonical UD arc material. Numeric
carriers remain proposals until admitted. Its four-slot beam retains competing
candidates with explicit integer fixture scores, exact edge agreement and a
source-owned pressure/cancellation frontier. Raw beam and frontier records are
proposals; active hypotheses need native graph and shared-basis admission before
truth publication.
This profile establishes neither a learned scorer nor ordinary kernel parsing
or prosody.

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

## Exact realization coverage

`coverage.conduit` owns finite native `LanguageCoverage` and `LanguageRequest`
contracts. Languages and explicit language/variety relationships are separate
sets. A declaration names a revision, evidence reference, exact supported
identities and optional provider-private mapping rows. It describes one
concrete Back/artifact; it is not portable catalog availability. Omitted or
empty coverage is undeclared. No provider name, language prefix or detection
hypothesis supplies missing coverage.

A portable Kind's `RealizationRequirement` law binds an exact structured
request configuration field to a domain-owned coverage property. Core retains
only finite typed realization properties, with no Language vocabulary. The
planner checks the Language law after Fore compatibility and before ordinary
resource/preference selection. Default, policy, characteristic and fixed
placement paths share that gate. Unsupported requirement profiles refuse;
they do not silently bypass admission.

A request may be language-sufficient or require its exact variety. A Back can
also declare variety sensitivity, requiring an explicitly supported variety
regardless of a language-sufficient request. There is no implicit broader
variety, English substitution or translation path. An explicit semantic
transformation must be authored as different work.

The selected declaration is copied into the Plan fingerprint. Preparation
compares it with the current Host/Boot offer. Planner inspection and Patchbay
`language_realization_details` expose requests and candidate decisions,
including coverage revision and variety sensitivity; ordinary refusal text
retains requested identities without dumping the full candidate table.

The shipped four-token specimen authors native `LanguageText` material. Its
exact Language identity supplies the language-sufficient realization request;
an annotation request is checked against incoming material before analysis.
The canonical material carrier admits 4096 UTF-8 bytes; this bounded four-token
reference retains its separate 1024-byte preparation refusal. It advertises
limited English fixture coverage on std and browser hosts. This proves exact
selection for that specimen, not grammar accuracy or broad English support.
Native English utterance preparation requires the declared experimental
pronunciation variety; a different or missing variety refuses before play.
The waveform renderer remains separate from that text frontend's linguistic
coverage.

Hosted eSpeak and Whisper discovery produces undeclared offers until a supplied
finite coverage declaration binds the exact provider artifact. Portable
Language/Variety requests map through explicit declaration rows to the private
voice or model-language argument. Changing provider source invalidates that
binding. Recognition retains each admitted request and declaration under its
exact lowered node before play; language detection hypotheses do not supply
missing coverage. Retained equipment without coverage still decodes, but ordinary installation
validation refuses it. An explicit replacement or removal must preserve the
validated Host and Body identities while repairing that selection.

Repository proof enters through `cargo xtask make host`: the
`declare-speech-language` and `declare-whisper-language` entrances produce new
bounded declaration and request files. This records supplied Host metadata,
not pronunciation or recognition accuracy. See the
[runtime speech guide](../proof/runtime-speech.md) for declaration and synthesis.
Full consumer proof and stable acceptance remain open.

`LanguageParserStableDependencyAdmission` retains an independent Native parser
fact and its proposed portable dependency arc. The Source laws in
`semantics/language/parser_session_dependency.conduit` correlate the complete
query, dependent and governor token identities, analysis revision, head ordinal,
universal relation base and full subtype material. Explicit `text/material`
observation compares the parser's empty-capable subtype with the portable
optional nonempty subtype without retagging either nominal contract.
The generated binding enforces field constraints and every `where` invariant.

This admission proves correlation with the retained stable fact. It does not
prove contiguous commitment, protected playback, learned-model execution or a
complete linguistic session. Those require their own retained Source execution
and model/policy custody. The subtype conformance test uses an explicitly
amended Native fixture, rather than claiming a subtype prediction from the
learned base-relation scorer.

The opt-in `stable_vocative_continuation` Speech test consumes exact recorded
Native stable facts and the complete revision history. It reconstructs the
lexical tape, admits the portable arc against the retained fact, and prepares
only that dependent's pronunciation, linguistic prosody, formant realization
and playback tape while the Language source is still Partial. The sparse-arc
adapter retains the original token ordinal rather than renumbering it by the
spoken-word index. The proof records the dependency admission, utterance intent,
playback basis and rendered frame count. Its played acknowledgements are explicit
manual effect-owner fixtures: queued evidence does not commit, premature commit
is refused, and withdrawal after played evidence is refused. This does not prove
physical playback, neural realization or protection through a later parser
revision. Those boundaries remain separate even when the recorded parser run
itself has admitted a subsequent rebase.
