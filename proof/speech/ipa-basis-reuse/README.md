# Declared IPA scope reuse

Issue [#5330](https://github.com/dancxjo/conduit/issues/5330) uses existing exact
immutable locals to share the complete inventory, notation basis and both
binding tables across two transcriptions and one phoneme. The
[Source](../../../semantics/speech/examples/ipa/reusable-basis.conduit) contains
all original material; it imports no pack and requires no file authority at Play.

[Expansion](expansion.json) records the actual checked Source/Plot identities,
constructor Kind IDs, exact scoped configuration Types and encoded sizes.
Reproduce these identities and the adversarial proof through the supported
`cargo xtask check speech-ipa` suite. Its reusable-basis authoring target prints
the expansion; the installed-construction target executes all three typed
external Fore deliveries through the existing std Host/kernel and independently
re-admits their decoded values against complete planned original material.
The example is an authored Plot with external output ports, not a sealed
`conduit run` entry. The executable installed proof supplies those exact typed
boundaries; it does not project them through a renderer or synthesize audio.

The checked locals retain exact Native Types and immutable lexical lifetime.
Every constructor still validates whole inventory membership. Missing, duplicate,
cyclic, foreign, stale, partial, conflicting, differently typed and overbudget
material refuse before publication. Renaming a local preserves configuration
material while retaining a new Source identity; a coherent new revision changes
encoded results. The product human/JSON diagnostic test points at the original
basis declaration rather than the alias at its use site.

Thumb and WASM gates prove compilation of shared alloc-only checking, not
execution on physical firmware or in a browser. The browser currently exposes
Human Source interaction admission rather than a separate Speech Source checker.
Universal notation and contextual phone realization remain distinct from
inventory-dependent phonemic identity; the exact future #5317 contract is in
[the notation design](../../../docs/design/speech-ipa-notation.md).

The local supported suite completed with **1,140 passing Rust tests across 26
targets**, nine existing environment-dependent std ignores (none IPA), and
successful WASM/Thumb compilation. Workspace formatting and patch hygiene pass. `cargo xtask ci pipeline unit lint`
also passes locked full-workspace all-target Clippy with warnings denied.

This is development evidence. Required Candidate, integration and automated
accepted-main receipts must exist before the owning issue closes; GitHub owns
those subsequent source-head and check identities. No stable acceptance is
asserted by this packet alone.
