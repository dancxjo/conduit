# Documentation

Conduit is one project described at several levels: what it is trying to make
possible, what the current code does, how to use it, and the evidence for
particular results. Start with the question you have rather than reading every
document in order.

## Start here

| Question | Read |
|---|---|
| What is Conduit? | [Project introduction](../README.md), then [Why Conduit](../wiki/Why-Conduit.md) |
| How does one continuing computer work? | [Bodies, hosts, plans, and plays](../wiki/Bodies-hosts-plans-and-plays.md), then [architecture tour](../wiki/Architecture-tour.md) |
| Can I do something with it? | [Try Conduit](try-conduit.md), [handbook start](../wiki/Start-here.md), or [annotated Conduitese examples](../wiki/Conduitese-by-example.md) |
| What do the unfamiliar nouns mean? | [Glossary](../wiki/Glossary.md) |
| What is demonstrated today? | [STATUS](../STATUS.md) and [evidence guide](../wiki/Evidence-and-proof.md) |
| What is still proposed? | [Roadmap](roadmap.md) and the owning [issues](https://github.com/dancxjo/conduit/issues) |

The [handbook source](../wiki/Home.md) is rendered by the published
[handbook](https://dancxjo.github.io/conduit/handbook/).
The separate GitHub wiki is a signpost, not another independently maintained
language manual.

## Learn and use the language

- [Conduitese overview](../wiki/Conduitese.md) and [examples](../wiki/Conduitese-by-example.md)
- [Current checked language surface](../wiki/Current-language-surface.md) and [feature coverage](../wiki/Conduitese-feature-coverage.md)
- [Units and quantities](../wiki/Units-and-quantities.md), [Types and state](../wiki/Types-and-state.md), and [plots and flow](../wiki/Plots-and-flow.md)
- [Try plots](try-plots.md) and the [reviewed plot inventory](../plots/README.md)
- [Creating models](../wiki/Creating-models.md): separate present demonstrations from proposed model journeys

## Understand the contracts

- [The Conduit canon](conduit-canon.md): durable intent and invariants, not a list of delivered features
- [Architecture reference](architecture/README.md): precise design boundaries
- [Body purpose and fulfillment](architecture/body-purpose-and-fulfillment.md): continuity, obligations, and lifecycle are distinct
- [Body time](body-time.md), [physical declarations](physical-declarations.md), and [repository ownership](repository-layout.md)
- [Design proposals](design/README.md): proposals and migrations, each with its own proof limits

## Run or develop a realization

- [Targets](../targets/README.md): browser, hosted machines, ConduitOS, and boards
- [Environmental observation Body](../bodies/environment/README.md): physical-device composition and scoped checkpoints
- [Contributor guide](../CONTRIBUTING.md) and [CI procedures](contributing/ci.md)
- [Proof and measurement guides](proof/) and [performance investigations](performance/README.md)
- [Website visual evidence](visual-evidence.md) and [current product identities](https://dancxjo.github.io/conduit/current-product.html)

## Evidence and history

- [Development audits](audits/README.md) and [documentation sweep (25 October 2026)](audits/documentation-sweep-20261025.md): point-in-time observations; they do not override current contracts
- [History and accepted milestones](history/README.md): checkpoint-specific receipts and retired designs
- [Reuse ledger](reuse-ledger.md): provenance and decisions about retained work
- [Documentation ownership and claim rules](documentation-policy.md): how to keep this corpus coherent

**When sources disagree:** the canon states intended invariants; the checked
source and its tests determine the present language; STATUS describes
implemented capability with scoped evidence; the owning issue tracks unfinished
work. Historical records remain true *about their named checkpoints*, not
about the latest `dev` or a future release.
