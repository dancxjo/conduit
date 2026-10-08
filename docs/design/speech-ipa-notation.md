# Canonical linguistic entities and IPA notation

Speech keeps phonemes, phones, their definitions and their occurrences distinct.
IPA is canonical notation on those typed entities. An IPA string alone cannot
establish inventory/variety membership, contextual realization or commitment.

The architectural reference is Tongues main
`b03702798db5a5e7d174278ac2fc1233f171b02e`: `phonology.rs` separates phoneme
intent from realized phones; `utterance.rs` carries the complete linguistic
intent; `plan_projection.rs` explicitly treats rendered IPA as a lossy projection.
Conduit preserves these distinctions through its existing Speech/Language Types.

## Ordinary Conduit authoring

The CLI authoring catalog installs the same checked Speech Types used by Native
bindings: `SpeechPhone`, `SpeechPhoneme`, occurrences, features, environments,
allophone rules, inventories and IPA notation profiles. See
[the source example](../../semantics/speech/examples/ipa/phones-and-phonemes.conduit).
Authored values pass through the ordinary parser, checker and expression
expansion. A phone cannot satisfy a phoneme port, even when their spelling agrees.

Definition construction and inventory admission are distinct stages.
`PreparedIpaInventory` retains the whole supplied inventory and checks explicit
phone/phoneme notation bindings against its declared inventory, variety and
notation revision. It retains original parsed spellings and evidence. A raw
record does not constitute admitted inventory membership or permission to speak.

## Supported notation

The recovered finite Version1 profile supports declared multicodepoint segments,
including `t͡ʃ d͡ʒ pʰ tʰ kʰ n̩ ã ã`, primary/secondary stress, length and syllable
boundaries. Unsupported combinations refuse rather than being approximated.
Complete partitioning refuses ambiguity; one byte or scalar is never presumed
to be one segment. Phonemic `/…/` and phonetic `[…]` delimiters are presentation.

No implicit Unicode normalization occurs. The composed/decomposed nasal spelling
pair requires an explicit authored alias, retaining source spelling and provenance.
Foreign inventories, varieties and revisions refuse. Provider/checkpoint codes
such as `ax`, `ch` and `p_aspirated` are not canonical IPA.

Profiles admit at most 256 units and 256 aliases. Transcriptions retain at most
4096 UTF-8 bytes and 256 occurrences; unit spellings retain at most 64 bytes.
Preparation allocates and does not claim bounded Flow execution.

Recovered source: `origin/codex/5212-common-intent` at
`6806f08fbb07723237441b97c93cebf5d1e72138`. Historical proof on that branch does
not prove the current candidate; current validation is recorded below.

## Proposed typed delimiter glyphs

Current glyph aliases resolve fixed unary/relational syntax to ordinary checked
Gears. They preserve source expansion evidence without arbitrary grammar changes.
A related extension could bind paired notation to a typed domain decoder:

```conduit
# Proposal only: this declaration grammar is not implemented.
with speech/phonetic-transcription as notation p "[" "]"
with speech/phonemic-transcription as notation p "/" "/"
```

The language would own delimiter recognition, lexical scope and expansion.
Speech would own supported IPA and the output Types. Expansion would retain
original spelling, spans, profile and normalization evidence. A qualified entrance
would disambiguate conflicts with sequence literals, division or operation paths;
failed decoding would never silently fall back to a competing interpretation.

A single-phone value, a single-phoneme value and a transcription sequence remain
different results. Universal phonetic notation needs no language inventory.
Phonemic notation and resolving notation into inventory references require an
explicit supplied inventory/variety basis. The extension must preserve ordinary
checked identities and use finite deterministic preparation rather than arbitrary
host evaluation. Broader utterance/syllable/prosody/acoustic work remains governed
by #5212; paired notation does not substitute for those typed meanings.

The nominal field-projection encoding correction is recovered from
`7817ee6098f85b996f44ade54ce5195d588eee96`. It preserves wrappers for nominal
leaf values; only plain primitive leaves use raw primitive bytes.

Run the focused conformance with `cargo xtask check speech-ipa`.

The preferred optional shorthand is `p[foˈnetika]` and `p/fonz/`, owned by #5317.
Bare collections and regexes retain their established syntax. Prefix/delimiter
bindings have one exact result Type; expected-Type inference cannot choose a
parser. Conflicts with a value named `p`, indexing or division must refuse with
an explicit qualified fallback. #5260 can finish through explicit constructors.
Phones are universal phonetic entities; phonemes require an explicit inventory.
An inventory may be assembled and domain-checked during preparation. Execution
planning may select support for it, but never invent its phonological contrasts.

The notation mechanism must be reusable for DSLs. A proposed regex consumer
`r/[A-Z]+/` uses the same prefix/delimiter contract. The existing slash form
and prefixed form must share the same portable pattern parser, flags, matching
semantics and admitted bounds. Existing `Text <= 64B ~ /[A-Z]+/` remains valid.
The proposed `Text <= 64B ~ r/[A-Z]+/` must check to the identical pattern constraint;
accepting typed pattern values at this consumer remains implementation work
under #5317, not a second regex engine.
Each binding has one output Type, bounded parsing, source custody and explicit
fallback. A non-speech fixture is required by #5317 before generality is claimed.


### Delimiter and payload escaping proposal

The common scanner owns where a literal ends; the domain parser owns the inner
language. Both receive the exact raw body and its source map. Prefixes are
recognized only when immediately adjacent to the opening delimiter, at an
admitted expression entry, under a unique explicit binding. Whitespace inside
the delimiters is payload. Whitespace between prefix and opener is ordinary
Conduit syntax. These lexical rules do not resolve collisions with indexing or
slash: conflicts still refuse with the qualified constructor suggested.

For the first version, use a shared backslash-aware delimiter scanner. A closing
delimiter preceded by an odd run of backslashes is quoted; an even run leaves
it active. A trailing escape or missing closing delimiter refuses at its source
span. Scanning does not consume or reinterpret payload escapes. The parser
contract declares which escapes it accepts and how they map to semantic values;
unknown escapes refuse rather than dropping backslashes. No generic string
unescaping is applied before parsing. This avoids double decoding regex escapes
such as `\.` or `\\` and gives other DSLs the same source-custody mechanism.

The regex consumer must route bare and prefixed forms through the same scanner
and parser. Today the scanner recognizes an escaped closing slash, but the
pattern parser rejects `\/`; literal slash is expressible inside a character
class, as `/a[/]b/`. The proposed `r/a[/]b/` must accept that same pattern and
`r/a\/b/` must retain the existing refusal. Any extension of accepted escapes
belongs in the shared parser, with conformance for both forms. Likewise, flags,
escaped backslashes, character classes and anchors must have identical meanings. Parser-specific suffixes such as regex `i` are part of the
reviewed parser contract and source identity; there is no universal flag syntax.
The existing regex scanner already accounts for character classes (so a slash
inside a class keeps its existing meaning); the prefixed form must use that same
reviewed lexical policy. Other DSLs do not inherit regex classes, comments,
interpolation or nesting. Such features require an explicit bounded lexical
policy in their own parser contract. Quoted delimiter escapes cannot make unsupported
IPA or invalid domain values admissible.

#5317 must verify malformed and unknown escapes, odd/even backslash runs,
escaped closers, adjacent literals, Unicode byte/scalar mappings, capacity
refusals and formatter idempotence, alongside an independent non-speech
consumer. The design remains proposed until that conformance passes.


`r/…/flags` denotes a checked portable pattern specification, rather than Text
or an executable host regex. The notation parser checks syntax and flags within
its declared preparation budget. A consumer such as a Text refinement supplies
the finite input bound before compiling the existing deterministic automaton.
Passing a pattern as typed Info must not bypass that admission step. Supporting
`r` does not imply support for regex constructs outside the current portable
subset; duplicate/unknown flags and unsupported expressions still refuse.


## Current development evidence

The ordinary [quoted-phone constructor](../../semantics/speech/examples/ipa/quoted-phone.conduit)
checks a single universal phone through `SpeechPhoneNotation`, including its
finite supported-spelling law. It needs no notation import, language inventory
or renderer. The Source conformance admits affricates, aspiration, syllabicity
and both declared nasal spellings; it refuses provider codes, orphan marks,
unsupported combinations, sequences and suprasegmentals at this single-phone
entrance. It verifies exact Native decoding, lossless CST source custody,
quoted-string highlighting and prepared/allocating evaluator parity.

Current focused proof passes 21 IPA tests and all 72 Speech library tests,
including universal phonetic events, nested phone checking and located Unicode
refusals. At the preceding quoted-phone checkpoint, 18 generic construction/
projection tests, all 313 Plot library tests, product catalog integration,
targeted lint and a thumbv6m no_std check also pass. The latter checks must run
again on the final implementation. This is development evidence, not stable
acceptance. Full quoted-transcription Source constructors remain unfinished; complete phonemic inventory resolution
now has its own focused proof below. #5260 remains open and its PR a draft.


The new `phonetic_from_ipa` preparation entrance accepts an exact quoted IPA
body without display brackets. It produces `SpeechPhoneticTranscription`, with
checked phone values and distinct stress/length/boundary events, a pinned finite
grammar revision, original spelling, provenance, and byte/scalar source spans.
It requires no language or inventory. The existing profile-scoped entrance
retains its complete supplied basis and explicit alias policy. `parse_located`
underlines unsupported scalars, invalid mark order and ambiguous suffixes;
capacity refusal underlines the first excess scalar without scanning an
unbounded body. These are preparation APIs; public qualified Source constructor
integration is still required before #5260 can close.


### Phonemic resolution under development

The complete phonemic entrance resolves parsed units against the explicit
prepared inventory's phoneme bindings. It refuses multiple complete partitions,
including both competing IDs with one spelling and competing groupings of units.
Length must belong to a declared phoneme binding: an inventory with only /a/
does not gain /aː/ from punctuation. Version1 keeps stress and syllable-boundary
marks as separate events and refuses bindings that absorb those marks into a
phoneme definition. It does not infer syllable or utterance structure.

The Native transcription retains the exact quoted body, generated display
notation, resolved references, original byte/scalar spans, unit ranges, the complete
phoneme binding table with its provenance, and the full notation basis. Its opaque executed receipt additionally retains the actual
inventory. Decoding a candidate Native record is not membership admission.
The quoted phonemic body is limited to 4094 bytes, leaving room for the profile
parser's two display slashes within its 4096-byte limit.

Five phonemic conformance tests pass, covering explicit long/short contrasts,
complete-parse ambiguity, alias evidence, source spans, foreign revision and
inventory substitution, unsupported bindings and capacity. Qualified Source
constructors remain required; these preparation APIs do not implement `p/.../` or `r/.../`.
