# Versioned source identity and complete lowering version 2

Status: C3 normative persisted-schema correction

This document corrects two version 1 identity boundaries without changing any
version 1 bytes or meaning:

1. source identity version 1 included caller-selected root state in the parsed
   `Panel`; and
2. lowered source version 1 omitted ordinary cords, composite-child
   relationships, exports, parameter bindings, and unresolved `using`
   constraints.

Version 2 keeps editable source, resolved selection, lowered semantic topology,
exact plans, runtime evidence, and presentation as separate identities.

## Version preservation

`semantic_source_hash_v1` and `lower_source` retain the frozen version 1
domains and results. `semantic_source_hash` remains the version 1 compatibility
alias. Existing grammar-v1, source-lowering-v1, plan-v1, and plan-v2 fixtures
are not reinterpreted.

New callers select source identity version 2 with
`semantic_source_hash_v2` or `semantic_source_hash_version(..., 2)`. They
select corrected lowering with `lower_source_v2` or
`lower_source_version(..., 2)`. Unsupported source or lowering schema versions
fail as `CND-SRC-011` or `CND-LWR-011`; they never fall back.

The grammar remains version 1. Source-AST schema version and grammar version
are distinct numbers.

## Authored source identity

Version 2 hashes every normalized authored AST fact already covered by version
1 under `conduit.panel-source/v2`, except `Panel.selected_root`. Comments,
trivia, spans, and caller selection do not participate.

Authored `root` declarations remain semantic source. Selecting one of them is
resolved input. Parsing the same source with `alpha` and `beta` selected
therefore yields:

- equal source-v2 identity;
- distinct resolved root-selection identity; and
- distinct corrected lowered identity.

For one declared root, automatic sole-root selection and explicit selection of
that same target have the same semantic selection identity. Their
`implicit-sole` or `explicit` mode remains explanatory metadata.

## Corrected lowered closure

`LoweredSourceV2` has explicit source-AST and lowering schema versions and
contains:

- a content-addressed selected-root input, when one exists;
- every node's path, exact contract, validated/defaulted configuration, and
  unresolved implementation/capability constraint;
- every ordinary top-level or composite cord, including complete finite flow
  policy;
- every composite-to-child ownership relationship;
- every explicit boundary export and parameter-to-child-config binding;
- the finite group members reconciled by specification 017;
- every finite pool specification; and
- a separate source map for all of those relationships.

Cord endpoints, export targets, and binding targets are fully scoped semantic
paths. Their identities include the relationship kind and every
plan-relevant field. Node identities include the presence and exact value of
an unresolved constraint. Corrected whole-closure identity uses the
`conduit.lowered-source/v2` domain over sorted typed fact hashes.

Lowering still performs no implementation selection, host observation,
artifact resolution, queue allocation, provisioning, or execution. An
unresolved constraint such as `using ready` is retained for exact planning
work owned by issue #61.

## Provenance

Imports, roots, composite definitions, nodes, node constraints, cords, exports,
and bindings retain exact one-based source spans. Every lowered relationship
maps to a `SourceOrigin` containing:

- canonical module URI;
- exact `sha256:` module-content identity; and
- authored span.

Content identity and spans are provenance, not semantic topology. Two sources
that differ only in trivia have different module-content identities but equal
source-v2 and lowered-v2 identities.

## Migration

A version 1 lowering record cannot be upgraded from its own fields because it
does not contain the omitted topology. `migrate_lowered_source_v1` therefore
requires the complete resolved, content-identified source graph and semantic
catalog:

1. reproduce version 1 lowering;
2. require its semantic hash and exact content-identified source map to equal
   the persisted record; and
3. re-lower the exact graph under version 2.

A mismatch or an origin-free v1 record fails as `CND-LWR-012`. This prevents a
stale, unrelated, or unverifiable source graph from being presented as a
migration. Readers may continue reading v1 without migration.

## Diagnostics

Version 2 adds:

| Code | Meaning |
|---|---|
| `CND-SRC-011` | unsupported source-AST schema version |
| `CND-LWR-011` | unsupported lowered-source schema version |
| `CND-LWR-012` | missing, stale, or ambiguous source graph for v2 reconstruction/migration |

Existing parser ownership remains unchanged. A dangling parameter binding
fails as `CND-SRC-003`; inaccessible or dangling child/export endpoints and
boundary bypass fail as `CND-SRC-009`.

## Conformance

`conformance/c3/source-lowering-v2.json` is normative. It covers independent
authored/root identities, implicit/explicit selection normalization, trivia,
constraints, cords, composite ownership, exports, bindings, nested imports,
exact content-identified origins, v1 reading, migration, unsupported versions,
stale migration, dangling relationships, and boundary bypass.

## Normative requirements

| ID | Obligation |
|---|---|
| SL2-001 | Preserve every source and lowering version 1 identity and fixture |
| SL2-002 | Exclude caller-selected root state from authored source-v2 identity |
| SL2-003 | Carry selected root as explicit content-addressed resolved/lowered input |
| SL2-004 | Normalize implicit sole-root and equivalent explicit selection semantically |
| SL2-005 | Retain every unresolved node constraint without selecting an implementation |
| SL2-006 | Retain every ordinary cord and complete finite flow policy |
| SL2-007 | Retain composite-child, export, and configuration-binding relationships |
| SL2-008 | Make every plan-relevant topology or constraint change alter lowered-v2 identity |
| SL2-009 | Keep exact module content identity and spans in separate source-map provenance |
| SL2-010 | Compose with versioned finite port-group and pool lowering |
| SL2-011 | Migrate v1 only by verified re-lowering of the exact resolved source graph |
| SL2-012 | Reject unsupported, stale, dangling, and boundary-bypassing inputs explicitly |
