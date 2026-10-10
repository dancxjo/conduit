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
