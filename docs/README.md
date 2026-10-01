# Documentation

Choose the path for what you want to learn or do. Current guidance lives here
and beside the code it describes; old checkpoints have a separate history index.

## Learn the model and language

1. [Project introduction](../README.md): purpose and the first hosted command
2. [Why Conduit](../wiki/Why-Conduit.md) and [architecture tour](../wiki/Architecture-tour.md): meaning, realization, and continuity
3. [Conduitese](../wiki/Conduitese.md) and [annotated examples](../wiki/Conduitese-by-example.md): learn the language through source
4. [Glossary](../wiki/Glossary.md): short definitions grouped by role

The [handbook](../wiki/Home.md) is also published as the
[GitHub wiki](https://github.com/dancxjo/conduit/wiki).

## Run, build, and inspect

| Task | Guide |
|---|---|
| See the product before building | [body Workspace](https://dancxjo.github.io/conduit/workspace/), [ConduitOS journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/) |
| Run hosted, browser, native workbench, or ConduitOS | [Try Conduit](try-conduit.md) |
| Read, check, and run examples | [Try forms](try-forms.md), [reviewed collection](../forms/README.md) |
| Choose a product or target | [Products](../products/README.md), [targets](../targets/README.md) |
| Build a body or host artifact | [body building](body-building.md), [host make](host-make.md) |
| Understand birth, lifecycle, and inspection | [Self-hosted biography](self-hosted-biography.md), [Patchbay](../products/patchbay/README.md) |
| Run a private remote rendezvous | [User-operated relay](user-operated-relay.md) |
| Export a package and provenance | [Supply-chain export](supply-chain-export.md) |
| Retain or reproduce visual proof | [Visual evidence](visual-evidence.md) |

## Look up an exact contract

- [Current language surface](../wiki/Current-language-surface.md): implemented spellings, examples, and proof limits
- [Language topic reference](../wiki/Home.md#reference): flow, types, terminals, effects, and construction
- [Architecture reference](architecture/README.md): contracts grouped by responsibility
- [Canon](conduit-canon.md): enduring direction and architectural invariants
- [Repository map](repository-layout.md): which owner a change belongs to

## Contribute and verify

- [Contributing](../CONTRIBUTING.md): setup and the ordinary PR workflow
- [CI guide](contributing/ci.md): admission, combined integration, and promotion
- [Local build storage](local-build-storage.md): build headroom and storage management
- [Proof dependency boundary](proof-dependency-boundary.md) and [browser proof](../proof/browser/README.md): validation ownership and tooling
- [Working agreement](../AGENTS.md) and [environment stewardship](contributing/agent-operations.md): contributor/agent constraints
- [Security acceptance](security/adversarial-acceptance.md): adversarial proof boundaries

## Check status or history

- [Current status](../STATUS.md): development capabilities and what their evidence proves
- [Roadmap](roadmap.md): unfinished and paused work
- [Current product truth](https://dancxjo.github.io/conduit/current-product.html): exact development, release, publication, and receipt identities
- [History](history/README.md): retained milestone, design, benchmark, and physical records
- [Reuse ledger](reuse-ledger.md): recovered ideas and reviewed provenance

## Maintain one home for each fact

The root README is a short entrance. The handbook teaches and defines the
language. Cross-cutting workflows live in `docs/`; detailed contracts live in
`docs/architecture/`; product, form, body, mechanism, and target guides stay
beside their owners. `STATUS` summarizes capability and proof limits, while the
roadmap points to unfinished work. Historical checkpoints belong under
`docs/history/`, not in the current reading path.

Edit wiki pages in repository `wiki/`. The publication workflow mirrors `dev`
to the separate GitHub wiki; direct wiki edits do not become a second source.
A draft PR is not a wiki publication or accepted release.

Keep commands on `conduit` or `cargo xtask`, link changing inventories rather
than copying counts, and distinguish checked syntax from proposals and target
execution proof. Preserve legal notices, third-party attribution, governance,
and unique evidence when retiring duplicate guidance. Git history retains
superseded prose; current pages should answer current reader questions.
