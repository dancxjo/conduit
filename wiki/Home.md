# Conduitese canon

> **Canonical reference.** These pages were migrated from issue [#4109](https://github.com/dancxjo/conduit/issues/4109) on 2026-09-29. The wiki is now the readable language reference; implementation tickets remain evidence of conformance and provenance.

This issue is the **authoritative current specification for Conduitese source and its core checked-language laws**.

Until an explicit successor canon issue replaces or amends a rule here:

1. **This issue wins over current parser behavior, old examples, stale fixtures, and closed design tickets.**
2. The current implementation is evidence of implementation status, **not** the definition of the language.
3. Closed issues linked below are design provenance. They are not parallel authorities.
4. If canonical syntax is not implemented, that is an **implementation gap**. Do not silently use legacy syntax.
5. If a semantic law is canonical but its surface spelling is marked **UNFROZEN**, do not invent a spelling in production source or a test-local fake catalog.
6. A test may construct narrow fixtures, but it may not manufacture missing production semantics and then claim the language supports them.

> **Canonical-but-unimplemented means “fix the implementation or expose the blocker,” never “fall back to whatever parses.”**

This issue is intentionally kept open as the living language contract. Ordinary language implementation work should be small child/micro tickets referenced from the current vertical that needs it.

## Read the canon

- [[Current language surface|Current-language-surface]] — frozen authored spellings and the latest surface settlement.
- [[Architecture]] — vocabulary, semantic paths, fore/back boundaries, and architectural altitude.
- [[Forms and flow|Forms-and-flow]] — forms, completion, cords, expressions, routing, filters, and source gears.
- [[Types and state|Types-and-state]] — finite bounds, type identity, temporal values, keep/data, structures, quantities, variants, and resources.
- [[Terminals and concurrency|Terminals-and-concurrency]] — close, abnormal terminals, cancellation, multi-input timing, fan-out, and pressure.
- [[Effects and realization|Effects-and-realization]] — inferred effects, realization invariance, replay, movement, fusion, and memoization.
- [[Packs, hosts and bodies|Packs-hosts-and-bodies]] — the shared source language, generics, packs/imports, host construction, and body construction.
- [[Canon governance|Canon-governance]] — implementation status, review law, conformance gates, amendment rules, and provenance.

## How to read this wiki

The wiki defines language meaning. Parser behavior, fixtures, examples, and closed design tickets are evidence of implementation history, not competing authorities.

When a canonical rule is not implemented, treat it as an implementation gap. Do not silently substitute legacy syntax.
