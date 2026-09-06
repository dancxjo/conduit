# Repository ownership map

Place material by the contract that owns it. File type, reuse count, and the activity that produced a file do not determine its owner. This guide defines placement; the [canon](conduit-canon.md) defines architecture, and [STATUS.md](../STATUS.md) records accepted executable proof. Issues #2275–#2279 and #2282 own this layout migration; candidate paths alone are not stable acceptance evidence.

## Placement law

| If it… | Owner | Does not belong here |
|---|---|---|
| Defines universal Form, Body, Plan, Play, kernel, or identity machinery | `architecture/` | Product state, concrete devices, proof-only scenarios |
| Defines portable Host-neutral meaning | `semantics/` | DOM, sockets, boot choices, credentials |
| Realizes a reusable protocol, device contract, or lower mechanism | `mechanisms/` | Speculative sharing without an independent contract |
| Generically manufactures machinery | `fabrication/` | Board-specific build policy or a concrete Body |
| Exists because of a browser, OS, board, or machine environment | `targets/` | Tour routing or another named product's state |
| Is a named human-facing product | `products/` | Generic Host truth or a concrete robot composition |
| Is a concrete Body composition | `bodies/` | A generic framework or a renamed proof fixture |
| Is a canonical reviewed authored Form/program | `forms/` | Proof-only samples or Host resource bindings |
| Exists to establish a claim | `proof/` | Fixed facts required by a production path |
| Exists to develop, build, or validate this repository | `tools/` | Target-specific setup or product-owned staging |
| Explains the project | `docs/` | Product runtime assets |
| Carries the public Pages landing entrance | `site/` | A product implementation or a second application runtime |

Root Cargo metadata, toolchain/configuration files, licensing and contributor guidance may remain at root. Retired `apps/`, `examples/`, `tour/`, `profiles/`, `scripts/`, `assets/`, `tests/`, and `xtask/` buckets have no current ownership role.

## Products, Bodies, and Forms

The current products are `products/conduit`, `products/tour`, `products/creche`, and `products/patchbay`. Conduit owns the installed CLI entrance. Tour owns guided executable learning, its Form Gallery, and authored lessons under `content/`. Crèche owns reviewed Body creation and lifecycle interaction. Patchbay owns the workbench and its native/browser manifestations. There is no reserved empty `products/book`: historical Book routes and saved-state compatibility are explicit Tour migration boundaries.

`bodies/pete` is a concrete robot Body, with its own composition and configurations. The embodied-house specimen in #2293 likewise belongs under `bodies/<specimen>` when implemented; this layout does not prebuild a house or turn that future specimen into a framework or product.

`forms/hello/main.conduit` is a canonical authored program. Each reviewed source has one `forms/<name>/` owner. A Form may own bounded assets, metadata, or fixtures, but these cannot duplicate semantic requirements or inject target/resource facts into authored meaning. Proof-only samples live in `proof/fixtures/forms/` or a narrowly owned package fixture.

`forms/inventory.toml` is the authoritative reviewed membership and proof inventory. Directory discovery validates membership; it never creates it. Tour Gallery, Crèche's reviewed initial selection, conformance, Body composition and other Forms consume those canonical sources through the existing inventory/checked-form paths. Consumer projections and finite selections are not new registries. Repository validation checks source paths against that same inventory.

Under #2291, one checked canonical Form may serve as a workload root or as one gear inside another Form through its face. Both uses refer to the same source and identity. There is no separate `subforms/`, `components/`, `modules/`, or second Form inventory. This placement rule does not claim a downstream composition proof before its owning issue establishes it.

Crèche birth selects ordinary initial active Forms into Body workload revision 0. Later revisions add or remove ordinary Forms. There is no current Seed repository category, `SeedId`, `BirthForm`, or `InitialProgramId` layer. Historical sources and evidence may retain historical terms; they are not current ontology.

## Browser and target boundaries

Tour source, state, actions, routing, style and application descriptor live in `products/tour/browser/`. Crèche owns the equivalent files in `products/creche/browser/`. Patchbay's browser package is `products/patchbay/html/`; its native icons are in `products/patchbay/native/assets/`. Package names do not need to change merely to make directory spelling uniform.

The browser Host owns generic package admission/loading, DOM and storage effects, identity, membership and bounded presentation. Renderer-neutral UI meaning belongs in `semantics/presentation`. Product state is never moved back into Host assets to make staging convenient. Explicit package dependency declarations select finite bytes from their real owners. See [browser product source ownership](browser-product-source-ownership.md).

[Target-family ownership](../targets/README.md) defines the optional `host`, `runtime`, `offers`, `fabrication`, `firmware`, `deployment`, `profiles`, `tools`, and `proof` responsibilities. No target gets empty directories for symmetry. Target Host examples live under `targets/<family>/profiles/`; Pete configuration belongs in `bodies/pete/profiles/`; proof-only topologies belong in `proof/fixtures/bodies/`. Target setup and credential/flash helpers belong in `targets/<family>/tools/`.

Conduit product integration tests live in `products/conduit/tests/`. Package tests stay with their package. Repository-wide proof suites live under `proof/`; Playwright metadata and dependencies belong in `proof/browser/`. Target-local proof appliances remain under their exact target only when their manufacturing/bring-up contract requires it.

Documentation diagrams belong in `docs/assets/`. Product staging belongs in the respective `products/<name>/tools/`; landing-page staging belongs in `site/tools/`. Generic CI lifecycle and artifact helpers belong in `tools/ci/`. These are distinct responsibilities, not a replacement support dump.

## Reuse and dependency direction

**Reuse is not an owner.** Do not create root `shared/`, `common/`, `utils/`, `misc/`, `components/`, `modules/`, or `libraries/` because several consumers use something. Identify the stable contract: portable presentation goes to `semantics/presentation`, browser manifestation to `targets/browser`, protocol machinery to `mechanisms`, product actions to their product, and canonical authored programs to `forms`. A small test fixture shared within one package can remain with that package's tests.

Architecture and semantics should not depend on products, concrete targets, or proof packages. Products and Bodies compose lower owners; Forms describe meaning. Targets realize lower contracts and admitted product requirements. Proof may depend on what it proves. A fixed proof fact required by production is a boundary defect, not evidence that proof is a reusable runtime library. These are review rules, not a forced global Cargo DAG or permission for speculative extraction.

## Entrances and guardrails

Use `conduit ...` for public product workflows and `cargo xtask ...` for repository development, validation, demonstrations, and hardware proof. Moving xtask beneath `tools/` and Playwright beneath `proof/browser/` changes no public command. Internal shell, Node and Cargo package commands are implementation details of those entrances.

The fast repository taxonomy test runs with the dependency-light CI dispatcher in ordinary classification, and with the workspace test gate reached through `cargo xtask check workspace`. It uses tracked Git paths, rejects retired root buckets and product source beneath the generic browser Host, and checks canonical Form paths against `forms/inventory.toml`. Harmless untracked local directories are outside its input. Deterministic negative cases cover rejected ownership, while existing inventory/conformance and browser package tests establish deeper contracts. Structural checks neither police source keywords nor claim execution, physical/HIL, or stable-main acceptance.
