# Prepared LanguageLexicalTape materialization prerequisite audit

Read-only Source/type and generic generator audit; no build, schema/NativeID
change, manual parser/policy mirror, training or production edit. Existing
prepared lexical admission is canonical law custody, not an owned Native
materializer. Native admission success alone does not make Vec/String creation
allocation-free inside outerPlay.

## Exact authored closure

LanguageLexicalTape = {source:LanguageTextRevision,profile:LanguageLexicalProfile,
tokens:sequence LanguageLexicalToken<=128}.

- source→LanguageTextRevision: material LanguageText; sequenceU64;
  prior optional LanguageTextPriorRevision; stable_prefix optional U32in0..4096;
  finality LanguageTextFinality(partial/final); provenance LinguisticDerivationProvenance.
- LanguageText→LanguageTextId<=64B,LanguageTextRevisionId<=64B,LanguageId<=32B,
  text<=4096B. LanguageTextPriorRevision→revisionID,sequenceU64.
- profile→identityText<=64B,LanguageId,entries sequence LanguageLexicalEntry<=64,
  provenance LinguisticDerivationProvenance.
- entry→surfaceText<=256Bnonempty,candidates sequence LanguageLexicalCandidate1..4.
- candidate→lemmaText<=128Bnonempty,LanguageLexicalPos17nullarycases,
  morphology sequence LinguisticTokenFeature<=8.
- feature→name/value Text<=1024Bnonempty.
- token→LinguisticTokenIdentity,TextSpan,surfaceText<=256Bnonempty,
  LinguisticTokenCategory(word/punctuation),LanguageLexicalCompleteness(complete/
  trailing_partial),candidate sequence<=4,prior_occurrence optional tokenidentity.
- tokenidentity→ordinalU64,textID,revisionID.
- TextSpan→LinguisticOffsetBasis(unicode_scalar/utf8_byte),start/endU64 with
  start<=end,textID,revisionID.
- provenance→deterministic_rule|library|model, EACH carrying LinguisticEvidence
  record{implementationText<=1024Bnonempty,revisionText<=1024Bnonempty}.

Inline refinements/optional wrappers carry their actual checked metadata and
must be resolved from the owner document, not recreated by matching field names.
No ResourceRef occurs in THIS exact LanguageLexicalTape closure. Resource lexicon
snapshot provenance/lease may be needed by the arrival controller separately,
but externalResource materialization is NOT a minimal prerequisite for this
Native root. Do not broaden generic work on that speculation. Likewise
LinguisticOptionalText and LinguisticTokenFeatureSlot are different legacy token
types, not reachable from this lexical tape; do not add them to required closure.

## Current generator seam

`generate_prepared_storage.rs` supports records, nominal wrappers over supported
representations, primitive fixed collections<=256, bounded text and nullary variants.
It rejects Sequence/optional/external shape at default arm, and payload variants
at its explicit nullary-only test. dependency() requires exact owner metadata
and rejects conflicting nested schema. emit.rs prepares fixed slot fields,
String::with_capacity for EACH text slot; step decodes one slot; finish_retained
moves private staged fields into Native and retains original decode gate allocation.
Current limits include4096slots and aggregate text<=65536. These are real blockers
for naive full-capacity lexical expansion even after sequence syntax is accepted.

## Smallest generic extensions (in dependency order)

1. Bounded sequence staging: retain admitted canonical length; prepare bounded
   element storage beforePlay; visit only present elements with exact element
   Native identity/contracts; move into existing BoundedSequence<T,MAX> without
   push-triggered growth. The actual BoundedSequence owner uses Vec capacity,
   so preparation must reserve it, not collect at finish. Zero/max/nested cases,
   malformed length, overflow, pressure/cancel and moved partial-storage recovery
   need generic proofs. Do not build all slots by copying a Language host loop.
2. Payload variant/optional staging: tag decoded/validated first; branch-owned
   preallocated payload records/primitives, absent/some distinctions preserved;
   only active branch materialized. Generic optional uses core optional schema
   and RustOption mapping, not an invented Language enum. Support provenance's
   three record-payload variants and prior/stable_prefix/prior_occurrenceOption.
   No dropping inactive allocated branch resources insidePlay: retain spent
   storage alongside completed Native/decode gate, as existing installed completion
   already does for borrowed preparation resources.
3. Exact bounds/geometry: nested sequence and optional leaf-step accounting must
   be finite and match actual admitted canonical traversal; cancellation/failure
   returns original staging custody. Native admission witness exact document/type/
   canonical equality remains required. Generated construction is materialization
   of proved values, never new law exemption or manually forged accepted flag.

ExternalResource support is not needed for this root; bounded recursive sequences
and payload variants are. Nonprimitive fixedarray support is not used here either.
Generated metadata collection plus tests should expose unsupported dependency
precisely; no Source semantic IDs or authored laws should change for generator-only
support additions.

## Capacity issue cannot be silently skipped

Full declared profile has64*4*8 morphology pairs and token tape128*4*8 pairs.
Just maximum name/value text capacities at1024B EACH consume
(64+128)*4*8*2*1024 =12582912bytes, BEFORE lemmas/surfaces/identities/source/provenance.
This already exceeds65536aggregate generator text budget and likely4096slots.
Canonical input admission may select a<=65536B actual value, but that does NOT
justify allocating worst-case capacities at every possible field or assuming
all morphology empty. A runtime short actual value cannot authorize allocation
duringPlay to resize owned Strings/Vecs later.

Root must choose an explicit generic preparation capacity model: either a reviewed
larger staging budget reflecting fullschema maxima, or a generic prepared capacity
layout selected BEFOREPlay for a bounded admitted subset (sequence slots and
per-text capacities explicitly retained; larger actual values refuse pressure).
The latter can preserve exact NativeID/laws without claiming all values of that
schema are supported by this installed profile. It must cover every nested
allocation, including embedded repeated profile entries. Known frozen profile
can be prepared beforePlay, but generated Native Tape owns it byvalue; an Arc
shortcut would change its Native representation and cannot be assumed. No
layout chosen here and no generic limit raised. Supporting shape alone is not
sufficient to claim this arrival path allocation-free.

## Required proof before ownercreation insidePlay

Actual generic prepared materializer receives the same admitted canonical lexical
witness, matches exact Native closures and fills all supported fields. Preallocate
its complete chosen profile beforePlay; prove zeroalloc/dealloc during incremental
step/finish, retain spent arrays/unused variant slots/decode gate, compare completed
Native reencoding OUTSIDEPlay byte-for-byte with original witness. Include nested
morphology, provenanceeachcase, optionalnone/some, zero/max tokens/candidates,
foreignSource/type/witness, undersized capacities and pressure/cancel without
partial ownerpublication. Arrival/rebase controller must also retain lexical
snapshot/provenance lease, epoch identity, machines/admissions/materializers;
materializing Tape alone does not grant Current or provide complete transaction.
No learned score, host tokenization policy or #5755 expansion belongs in this
prerequisite. Root implementation remains owner of that future slice.
