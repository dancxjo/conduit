# Scoped typed glyphs

Implementation work for [#5317](https://github.com/dancxjo/conduit/issues/5317).
This document describes the development implementation; protected integration
and stable-publication acceptance remain required.

A notation import binds an explicitly shipped glyph-family declaration:

```conduit
with speech/ipa/notation as ph
with text/pattern/notation as r
```

The IPA declaration is stored in `semantics/speech/ipa/notation.conduit`, shipped
by `semantics/speech/ipa/pack.conduit`. Its square branch produces a
`SpeechPhoneticTranscription`; its slash branch produces a
`SpeechPhonemicTranscription`. Each branch names the existing checked constructor.
The local name `ph` is an alias, not a reserved word or an ambient inventory.

The pattern declaration is stored in `semantics/text/pattern/notation.conduit`.
Its slash and double-square spellings produce the same checked
`PortablePatternSpecification` through `text/pattern-from-source`:

```conduit
with text/pattern/notation as r
plot patterns {
    ascii = r/^[A-Z]+$/i
    unicode = r⟦^[A-Z]+$⟧i
}
```

`r/…/` is the plain-keyboard entrance for `r⟦…⟧`. The formatter preserves the
spelling the author chose. Neither representation changes the portable pattern
subset, flags or escape law. A literal slash uses `[/]`; `\/` retains the
existing portable-parser refusal. A Text refinement supplies its finite input
bound before a pattern can become a checked constraint:

```conduit
with text/pattern/notation as r
plot labels (
    >> value: Text <= 64B ~ r/[A-Z]+/i
) {
}
```

IPA glyphs retain the explicit checked context selected by their import, such as
`using {provenance: evidence}` for an ordinary immutable local named `evidence`.
Phonemic admission additionally needs its complete inventory, binding, variety
and revision context. An import never supplies that authority implicitly.
The examples in `semantics/speech/examples/ipa/` retain the fully qualified
constructor entrances and complete request/basis values.

A glyph result can be stored in an ordinary immutable local and used by a pure
expression. Expansion retains its checked structured value and exact Type as a
portable constant, preserving the reference span without reparsing the payload.
Only referenced locals are retained in the expression program.

## Grammar and collision rules

An imported alias is recognized only with its adjacent, declared opening
delimiter. The branch fixes the closing delimiter, scanner policy, bounded
payload parser and result Type; expected-Type inference never selects a branch.
The lexer retains raw payload bytes. The domain constructor decides whether
escapes, flags and the resulting value are valid.

Version one recognizes only the reviewed square (`[…]`), slash (`/…/`), angle
(`⟨…⟩`) and double-square (`⟦…⟧`) pairs. A family declares its subset; metadata
cannot install arbitrary punctuation, combining marks, invisible delimiters or
bidi controls. A catalogue contains at most 64 families, each with at most four
branches and a payload bound of at most 4096 bytes. Portable patterns use only
the slash or double-square branches. These are byte limits, not character limits.
Unicode payloads retain their exact spelling without NFC/NFKC rewriting.

| Source form | Meaning or refusal |
| --- | --- |
| `ph[…]` | Phonetic transcription, with explicit checked provenance. |
| `ph/…/` | Phonemic transcription, requiring the complete explicit checked basis. |
| `r/…/i`, `r⟦…⟧i` | Declared aliases for one portable pattern specification. |
| `[a, b]`, `in [a, b]`, `~ /…/i`, `!~ /…/i` | Existing collection, membership and bare-pattern syntax. |
| `ph / x / y` with `ph` imported | Refusal: a bound introducer requires an adjacent declared delimiter. Use the qualified constructor. |
| A local value, Type, Gear or second import named `ph` | Refusal at the conflicting lexical binding; rename the notation alias or use the qualified constructor. |
| An undeclared or visually similar delimiter pair | Refusal; no implicit normalization or confusable substitution. |
| Missing closer, invalid escape/flags or excess payload | Bounded scanner or domain-admission refusal, retaining authored source locations. |

Multi-line payloads retain their original bytes and source spans. Punctuation
inside a complete glyph does not become an outer Plot statement or comment.
Importing notation leaves direct Quantity literals such as `440Hz`, `250ms`,
`21°C` and `640px` native and import-free; `21C` still refuses.

## Formatting

`conduit fmt source.conduit` prints formatted Source. It changes block
indentation and exterior horizontal whitespace, preserving glyph payloads,
delimiter choice, quoted values, bare patterns and comments. It does not write
the input file. `conduit fmt source.conduit --check` emits no rewritten Source
and refuses when formatting would change the file.

```sh
conduit fmt source.conduit > formatted.conduit
conduit fmt formatted.conduit --check
conduit check formatted.conduit
```

Formatting parses before and after rewriting and retains the finite Source
byte bound. Syntax formatting is separate from domain-value admission:
`conduit check` remains the entrance for complete semantic diagnostics.
Changed authoring bytes have a new Source identity; literal spelling, imported
family/parser identity, selected basis and ordinary checked values remain
truthful. Formatting never restores an old receipt against changed Source.

## Checked inspection

`conduit expand source.conduit --json` reports each admitted glyph's authored
bytes and spans, imported family identity, parser contract, ordinary constructor,
exact result Type and checked value digest. `canonical_value_hex` contains the
existing Core structured-value encoding, including its exact Type; inspection
does not parse the payload again. Selected basis values appear once in
`glyph_contexts`; glyph basis entries reference their context index and digest.

`conduit expand source.conduit` and `conduit inspect source.conduit` also show
the authored and checked forms in plain text, including selected basis names
and identities. Native Type refinements are included alongside Plot literals.

The native Patchbay editor uses the same scoped parser and checked constructor
admission for initial Source, revision checks, composition edits and expansion.
Its standard catalog includes the shipped pattern and IPA families. Replacing
Source preserves authored bytes and requires a new revision check; an older
check cannot replace the current revision. Native editor conformance does not
establish browser execution or stable publication.

The installed browser profile uses scoped admission for execution, checked
Patchbay projection, reviewed resident inventory and multi-host planning.
Structured glyph constants reuse the ordinary prepared expression evaluator;
equality preserves exact Types, IEEE bit identity and quantity conversion laws
without allocating during evaluation. Local pinned Chromium proof exercises
ASCII/Unicode pattern equality, completed Plays and refusals before Play for
missing imports, invalid escapes and missing phonemic basis. Stable acceptance
still requires protected integration and publication evidence.
