# Library catalog inventory version 1

Issue: #159. Parent: #158.

## Boundary

`library/catalog-v1.json` is the checked inventory of every semantic node
published by the current registry. It is generated from exact contract
descriptors by `cargo xtask catalog-index`; unknown namespaces, duplicate
active identities, provider bundles for unknown contracts, missing fixture
owners, and stale generated output fail the gate.

Catalog membership means that a contract is known. It does not mean that a
provider is installed, initialized, current, admitted, or authorized.
`known_provider_bundles` records immutable implementation and artifact facts.
`current_provider_observation` is always `not-recorded-in-catalog`; current
availability belongs to a fresh host observation and exact plan.

## Classification and ownership

Every active entry has exactly one mechanical classification:

- `portable-standard` for cross-domain `std/`, `flow/`, `time/`, `state/`,
  `supervision/`, and retained `text/uppercase` semantics;
- `optional-host-boundary` for I/O, filesystem, storage, process, device,
  networking, transport, secret, crypto, compression, and evidence effects;
- `reusable-domain-package` for AI, knowledge, learned, media, robotics,
  spatial, and speech namespaces;
- `implementation-helper` for testing and observation helpers.

Provisional or duplicate spellings do not remain active beside their
replacement. The former `conduit.std/*` spelling is removed and has the exact
`std/*` replacement rule; the registry does not resolve it as an alias.

Each retained entry records its canonical identity and public spelling,
package artifact/export, schema and semantic hash, exact port/config type
references, catalog/compiler exposure, known provider bundles, fixture and
profile owner, structural-facet owner, lesson artifacts, and any
successor/deprecation/adapter facts.

## Lessons and presentation

Every exported contract points to a standalone and composition lesson
artifact. `published` means that the named artifact is present in the current
Tour; `required` is checked backlog owned by #160 and cannot be mistaken for a
shipped lesson.

`docs/library-tour-index.md` is generated from the same inventory.
Patchbay's `project_library_catalog` consumes that artifact read-only and
presents contract class/package, known bundles, observation boundary,
fixtures, and lesson status separately. Projection performs no discovery,
installation, loading, enrollment, or authority grant.

## Stable diagnostics

- `catalog-index` fails when a catalog identity has no explicit namespace
  owner/classification, when an active identity is duplicated, when a known
  provider targets no catalog contract, when a fixture owner is missing, or
  when either generated artifact is stale.
- Patchbay rejects malformed, oversized, duplicate, unknown-class, or
  observation-bearing catalog documents with `CND-PBY-014`.
