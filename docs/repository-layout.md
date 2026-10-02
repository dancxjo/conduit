# Repository ownership map

Place material by the contract that owns it. File type, reuse count, and the activity that produced a file do not determine its owner. This guide defines placement; the [canon](conduit-canon.md) defines architecture, and [STATUS.md](../STATUS.md) records capability and proof status. The source layout is organized around these owners; historical migration issues are #2275–#2279 and #2282.

## Placement law

| If it… | Owner | Does not belong here |
|---|---|---|
| Defines universal form, body, plan, play, kernel, or identity machinery | `architecture/` | Product state, concrete devices, proof-only scenarios |
| Defines portable host-neutral meaning | `semantics/` | DOM, sockets, boot choices, credentials |
| Realizes a reusable protocol, device contract, or lower mechanism | `mechanisms/` | Speculative sharing without an independent contract |
| Generically manufactures machinery | `make/` | Board-specific build policy or a concrete body |
| Exists because of a browser, OS, board, or machine environment | `targets/` | Tour routing or another named product's state |
| Is a named human-facing product | `products/` | Generic host truth or a concrete robot composition |
| Is a concrete body composition | `bodies/` | A generic framework or a renamed proof fixture |
| Is a canonical reviewed authored form/program | `forms/` | Proof-only samples or host resource bindings |
| Exists to establish a claim | `proof/` | Fixed facts required by a production path |
| Exists to develop, build, or validate this repository | `tools/` | Target-specific setup or product-owned staging |
| Explains the project | `docs/` | Product runtime assets |
| Carries the public Pages landing entrance | `site/` | A product implementation or a second application runtime |

Root Cargo metadata, toolchain/configuration files, licensing and contributor guidance may remain at root. Retired `apps/`, `examples/`, `tour/`, `profiles/`, `scripts/`, `assets/`, `tests/`, and `xtask/` buckets have no current ownership role.

## Products, bodies, and forms

`products/conduit` is the sole installed product and distribution entrance.
`forms/tour` owns resident tutorial meaning while `docs/journeys/tour` owns its
authored journey. `forms/patchbay` owns the resident inspect/edit/debug Form and
its workbench Mask contracts. Browser Workspace and Patchbay realizations live
under `targets/browser`; they consume Body, Form, Face, Mask, Plan, and Play
truth without creating separate product, scheduler, lifecycle, or authority
universes. The retired Tour, Crèche, Home, Workspace, and Patchbay product
shells have no reserved directories or compatibility packages.

`bodies/pete` is a concrete robot body, with its own composition and configurations. The unfinished embodied-house specimen in #2293 belongs under `bodies/<specimen>` when its concrete composition is added; its existing semantic work does not establish a persistent live House.

`forms/hello/main.conduit` is a canonical authored program. Each reviewed source has one `forms/<name>/` owner. A form may own bounded assets, metadata, or fixtures, but these cannot duplicate semantic requirements or inject target/resource facts into authored meaning. Proof-only samples live in `proof/fixtures/forms/` or a narrowly owned package fixture.

`forms/inventory.toml` is the authoritative reviewed membership and proof inventory. Directory discovery validates membership; it never creates it. Tour Gallery, the body Workspace's reviewed initial selection, the Crèche compatibility route, conformance, body composition and other forms consume those canonical sources through the existing inventory/checked-form paths. Consumer projections and finite selections are not new registries. Repository validation checks source paths against that same inventory.

Under #2291, one checked canonical form may serve as a workload root or as one gear inside another form through its front. Both uses refer to the same source and identity. There is no separate `subforms/`, `components/`, `modules/`, or second form inventory. This placement rule does not claim a downstream composition proof before its owning issue establishes it.

Zero-body bootstrap selects ordinary initial active forms into body workload revision 0. Later revisions add or remove ordinary forms. There is no current seed repository category, `SeedId`, `BirthForm`, or `InitialProgramId` layer. Historical sources and evidence may retain historical terms; they are not current ontology.

## Browser and target boundaries

The browser Host owns generic package admission/loading, DOM and storage
effects, identity, membership, and bounded Face realization. Renderer-neutral
human meaning belongs to Face; browser layout and interaction mechanism belong
to the selected Mask. Product state is never moved into Host assets merely to
make staging convenient. Explicit package dependency declarations select
finite bytes from their real owners.

The browser target owns the concrete Workspace carrier and Patchbay workbench
realization. Their portable meaning remains with resident Forms and semantic
owners; target placement grants no authority to invent a second runtime.

[Target-family ownership](../targets/README.md) defines the optional `host`, `runtime`, `offers`, `make`, `firmware`, `deployment`, `profiles`, `tools`, and `proof` responsibilities. No target gets empty directories for symmetry. Target host examples live under `targets/<family>/profiles/`; Pete configuration belongs in `bodies/pete/profiles/`; proof-only topologies belong in `proof/fixtures/bodies/`. Target setup and credential/flash helpers belong in `targets/<family>/tools/`.

Conduit product integration tests live in `products/conduit/tests/`. Package tests stay with their package. Repository-wide proof suites live under `proof/`; Playwright metadata and dependencies belong in `proof/browser/`. Target-local proof appliances remain under their exact target only when their manufacturing/bring-up contract requires it.

Documentation diagrams belong in `docs/assets/`. Target staging belongs with
its exact `targets/<family>/tools/` owner; landing-page staging belongs in
`site/tools/`. Generic CI lifecycle and artifact helpers belong in `tools/ci/`.
These are distinct responsibilities, not a replacement support dump.

## Reuse and dependency direction

**Reuse is not an owner.** Do not create root `shared/`, `common/`, `utils/`, `misc/`, `components/`, `modules/`, or `libraries/` because several consumers use something. Identify the stable contract: portable presentation goes to `semantics/presentation`, browser manifestation to `targets/browser`, protocol machinery to `mechanisms`, product actions to their product, and canonical authored programs to `forms`. A small test fixture shared within one package can remain with that package's tests.

Architecture and semantics should not depend on products, concrete targets, or proof packages. Products and bodies compose lower owners; forms describe meaning. Targets realize lower contracts and admitted product requirements. Proof may depend on what it proves. A fixed proof fact required by production is a boundary defect, not evidence that proof is a reusable runtime library. These are review rules, not a forced global Cargo DAG or permission for speculative extraction.

## Entrances and guardrails

Use `conduit ...` for public product workflows and `cargo xtask ...` for repository development, validation, demonstrations, and hardware proof. Moving xtask beneath `tools/` and Playwright beneath `proof/browser/` changes no public command. Internal shell, Node and Cargo package commands are implementation details of those entrances.

The fast repository taxonomy test runs with the dependency-light CI dispatcher in ordinary classification, and with the workspace test gate reached through `cargo xtask check workspace`. It uses tracked Git paths, rejects retired root buckets and product source beneath the generic browser host, and checks canonical form paths against `forms/inventory.toml`. Harmless untracked local directories are outside its input. Deterministic negative cases cover rejected ownership, while existing inventory/conformance and browser package tests establish deeper contracts. Structural checks neither police source keywords nor claim execution, physical/HIL, or stable-main acceptance.
