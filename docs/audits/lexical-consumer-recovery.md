# Supported lexical consumer recovery

For #5751, recover only the finite pronunciation-selection component from
`codex/4907-native-admission`, preserved head
`f3f702075ef96386537884dd5a6755191be21779`. The original branch still contains
other useful work and is not eligible for wholesale retirement. No parser,
training fixture or speech realization stack is copied into this successor.

The recovered Source relation selects among the original bounded lexical
alternatives using an explicitly supplied canonical dependency arc and reviewed
relation-to-POS policy. It verifies the exact analysis and original occurrence
identity against the immutable lexical tape. It does not infer syntax from words
or claim a parser produced the supplied arc. The old `other(unit)` literal is
migrated to current `LanguageLexicalPos.other` syntax.

A new Source-owned consumer inventory declares supported Language/Variety and
exact feature-name/value pairs. Preparation checks every retained candidate's
morphology before selection, keeps the complete original tape and inventory, and
retains Native basis, variety and feature-membership receipts. Unsupported
alternatives are refused rather than silently pruned. The consumer holds the
same original occurrence identity and retains all competing unscored analyses.
It does not retokenize or derive identity from the spelling `record`.

The fixtures supply noun/Number=Singular and verb/Tense=Present alternatives for
`record`. Explicit determiner or subject arcs select the appropriate candidate.
Positive tests check original identity, both retained alternatives, complete
feature receipts and serialization/re-admission. Negatives cover unsupported
feature/variety, five alternatives beyond the finite four-item bound, same text
identity with a foreign revision, stale analysis, wrong target, unsupported and
duplicated policy, and duplicate candidates.

Recovered selection Types and new consumer contracts use the existing language
catalog. Runtime Port tests declare the finite 8192-byte envelope explicitly;
large preparation schemas are not silently admitted without a finite limit.
This is checked linguistic preparation and supplied-arc consumer conformance,
not a new dependency parser, Flow playback or speech synthesis proof. Fourteen distinct focused tests pass (six existing lexical, five recovered
selection, three consumer tests). Strict Clippy with warnings denied passes.
