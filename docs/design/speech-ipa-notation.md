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
not prove the current candidate; current validation remains pending.

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
different results. Resolving any of them into inventory references requires an
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
`r/[A-Z]+/` uses the same prefix/delimiter contract, with regex-specific escape
and flag rules owned by that parser. Existing `Text ~ /[A-Z]+/` remains valid;
accepting `Text ~ r/[A-Z]+/` would be a separately checked consumer integration.
Each binding has one output Type, bounded parsing, source custody and explicit
fallback. A non-speech fixture is required by #5317 before generality is claimed.
