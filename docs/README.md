# Conduit documentation

Start with what you want to do. These guides separate the project's purpose,
current capabilities, practical workflows, and detailed design references.

## Start here

- [Project introduction](../README.md): the idea, working products, and a first command.
- [body Workspace](https://dancxjo.github.io/conduit/workspace/): birth or return to a body and use its resident tutorial and inspection forms.
- [Interactive Tour compatibility entrance](https://dancxjo.github.io/conduit/tour/): learn by running real forms in a browser.
- [ConduitOS visual journey](https://dancxjo.github.io/conduit/current/conduitos/x86_64/): 17 narrated emulator screenshots with runtime evidence.
- [Current status](../STATUS.md): what exists and the limits of its proof.
- [Roadmap](roadmap.md): current open work and paused campaigns.
- [Contributing](../CONTRIBUTING.md): setup, finding a change, checking it, and opening a PR.

## Build and use

| Guide | Use it for |
|---|---|
| [Try Conduit](try-conduit.md) | Local hosted, browser, Patchbay, and ConduitOS entrances |
| [Try forms](try-forms.md) and [form collection](../forms/README.md) | Examples, authoring, and checking reusable programs |
| [Product surfaces](../products/README.md) | body Workspace, compatibility/development surfaces, and the CLI |
| [Targets](../targets/README.md) | Platform setup, fabrication, and target-specific proof |
| [body building](body-building.md) | body-bound target artifacts and deployment boundaries |
| [host fabrication](host-fabrication.md) | PROFILE, BUILD, IMAGE, and fabrication packages |
| [Supply-chain export](supply-chain-export.md) | OCI transport, in-toto/SLSA provenance, and proof separation |
| [User-operated relay](user-operated-relay.md) | Provision and run one private outbound-only remote rendezvous path |
| [Visual evidence](visual-evidence.md) | Screenshots, provenance, reproduction, and publication |

## Understand the design

The [canon](conduit-canon.md) is the durable architectural explanation. The
[architecture index](architecture/README.md) groups the detailed contracts and
clearly identifies historical milestone designs.

| Topic | References |
|---|---|
| Runtime and hosts | [host architecture](host-architecture.md), [compute resources](r2-compute-resources.md), [timing](timing-profile.md) |
| Human interfaces | [Presenter boundary](presenter-hourglass.md), [input semantics](input-semantics.md), [browser application boundary](browser-application-presentation-boundary.md) |
| Ownership | [Repository layout](repository-layout.md), [browser product ownership](browser-product-source-ownership.md) |
| body evidence | [Self-hosted biography](self-hosted-biography.md), [body lifecycle contracts](architecture/body-lifecycle-waists.md) |

## Develop and verify

- [CI for contributors](contributing/ci.md): PR admission, development integration, and releases.
- [Local build storage](local-build-storage.md): disk-backed builds and storage management.
- [Proof dependency boundary](proof-dependency-boundary.md) and [browser proof](../proof/browser/README.md): where validation code belongs.
- [Contributor and agent rules](../AGENTS.md): shared implementation and collaboration constraints.

## Historical records

[Recorded acceptance milestones](history/accepted-milestones.md) preserve the
old status ledger, including its exact receipts and checkpoint-specific limits.
The [reuse ledger](reuse-ledger.md) preserves recovered ideas and provenance.
[CI impact benchmarks](ci-impact-benchmark.md) are measurements of their named
runs. [Candidate-evidence history](ci-candidate-evidence.md) retains the retired
CI design; the old [integration guide](integration-and-promotion.md) points to
the current contributor procedure. Historical statements are not current setup or capability instructions.

## Where documentation belongs

Keep one home for each kind of information:

- Root **README** explains why the project exists and how to start.
- **CONTRIBUTING** explains how to help; **AGENTS** holds implementation rules.
- **STATUS** summarizes current capability and proof limits; **roadmap** links to unfinished work.
- **`docs/` guides** explain cross-cutting workflows and concepts.
- **`docs/architecture/`** holds detailed design contracts and indexed historical designs.
- **Product, form, body, mechanism, and target READMEs** live beside their owners.
- **`docs/history/`** holds old milestone records; evidence stays with its provenance.

Prefer improving an existing guide to adding a new competing explanation. Link
to source or generated inventories for changing details such as catalog counts.
When a milestone completes, update the short current summary and retain its
proof history without pasting another implementation diary into the front door.
