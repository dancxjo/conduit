# Conduit specifications

The specifications distinguish facts that are easy to conflate:

1. semantic contracts;
2. concrete implementations;
3. host capability and resource observations;
4. editable `.panel` source;
5. exact resolved execution plans;
6. immutable execution evidence;
7. Patchbay presentation and mutable product projections.

The current documents are candidates, not a claim of ecosystem stability:

- [`000-c0-evidence.md`](000-c0-evidence.md) records the brownfield evidence
  that justifies extracting Conduit.
- [`001-c1-bootstrap-meta.md`](001-c1-bootstrap-meta.md) defines how later
  specifications and descriptors become normative and conformant.
- [`002-roadmap.md`](002-roadmap.md) defines the smallest proof sequence from
  the current one-shot executor to embedded and domain integrations.
- [`003-canonical-descriptor-v1.md`](003-canonical-descriptor-v1.md) freezes the
  allocator-free canonical descriptor bytes and semantic-hash vectors.
- [`004-directional-compatibility-v1.md`](004-directional-compatibility-v1.md)
  freezes reasoned compatibility queries, version direction, substitution, and
  migration identity.

The executable code is a conformance seed. Where it implements only a strict
subset, these documents say so explicitly.

## Normative language

**MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** carry their
ordinary standards-document meanings. A candidate requirement becomes stable
only when it has:

- a stable requirement identifier;
- a motivating case or explicit safety/portability reason;
- positive and negative fixtures;
- a conformance class;
- a stable diagnostic for rejectable behavior;
- at least one implementation, and ordinarily an independent reader or
  reference model.

## Dependency direction

```text
Tongues domain contracts ─┐
Netherwick contracts ─────┼──→ Conduit semantic and execution contracts
other domains ────────────┘

Patchbay ──→ Conduit authoring identities, diagnostics, plans, and evidence
Psyched ───→ provisioning and product orchestration around Conduit hosts
```

Conduit does not depend on those projects.
