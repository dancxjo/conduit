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
- [`005-type-contracts-v1.md`](005-type-contracts-v1.md) defines opaque
  domain-owned type references, hosted provider discovery, and explicit
  nominal, structural, and opaque comparison.
- [`006-port-config-contracts-v1.md`](006-port-config-contracts-v1.md) freezes
  complete port boundaries, directional diagnostics, and separate typed
  configuration/default/redaction semantics.
- [`007-bounded-flow-policy-v1.md`](007-bounded-flow-policy-v1.md) freezes
  finite item/byte capacity, pressure transitions, type-gated loss, evidence,
  and the allocator-free reference queue.
- [`008-lifecycle-cancellation-terminal-v1.md`](008-lifecycle-cancellation-terminal-v1.md)
  freezes lifecycle transitions, bounded hierarchical cancellation,
  deterministic terminal races, and replicated-child supervision.
- [`009-exported-composites-v1.md`](009-exported-composites-v1.md) freezes
  ordinary composite definitions, transparent exports, parameter bindings,
  recursive lowering, and logical/expanded provenance.
- [`010-scoped-authority-v1.md`](010-scoped-authority-v1.md) separates fresh
  host capability, effects, grants, exact bindings, delegation, revocation,
  and structurally redacted sensitivity handling.
- [`011-exact-execution-plan-v1.md`](011-exact-execution-plan-v1.md) freezes
  exact runnable-plan contents, canonical identity, portable validation,
  freshness, resource accounting, and bounded expansion/pool identity.
- [`012-immutable-execution-event-v1.md`](012-immutable-execution-event-v1.md)
  freezes append-only run evidence, causation/correlation/time separation,
  structural redaction, replay validation, and hosted NDJSON round-tripping.
- [`013-conformance-harness-v1.md`](013-conformance-harness-v1.md) freezes the
  language-neutral fixture inventory, NDJSON runner protocol, structured
  comparison results, deterministic seeds, and fixture review/version rules.
- [`014-panel-grammar-modules-v1.md`](014-panel-grammar-modules-v1.md) freezes
  `.panel` grammar version 1, lossless CST/source-AST separation, deterministic
  imports and roots, and bounded group/pool authoring forms.
- [`015-typed-source-lowering-v1.md`](015-typed-source-lowering-v1.md) freezes
  exact typed literals, schema validation and defaults, provenance-safe
  semantic lowering, cross-module source maps, and finite group/pool
  expansion.
- [`016-structured-diagnostics-v1.md`](016-structured-diagnostics-v1.md)
  freezes allocator-free diagnostic data, exact source spans, structural
  redaction, guarded fixes, lossless JSON, and hosted terminal rendering.
- [`017-port-groups-correlation-v1.md`](017-port-groups-correlation-v1.md)
  reconciles finite keyed/indexed port groups through source, lowering,
  plan-v2, exports, and evidence, and freezes distinct correlation identity
  allocators, scopes, lifetimes, and propagation.
- [`018-conduct-cli-v1.md`](018-conduct-cli-v1.md) freezes the canonical
  `conduct` command model, structured failure routing, stdout/stderr ownership,
  terminal and color policy, broken-pipe behavior, and measured dependency
  cost.
- [`019-source-lowering-v2.md`](019-source-lowering-v2.md) preserves frozen v1
  identities while moving root selection out of authored source identity and
  retaining complete cords, composite relationships, constraints, and exact
  provenance in corrected lowering v2.
- [`020-conduct-output-v1.md`](020-conduct-output-v1.md) freezes reproducible
  completions and manuals, independent result and diagnostic selectors,
  finite result JSON, ordered run NDJSON, quiet/verbosity, bounded progress,
  and machine-safe stream behavior.
- [`021-safe-inspection-v1.md`](021-safe-inspection-v1.md) freezes bounded,
  marker-only, non-executing inspection across source, lowering, plan,
  evidence, diagnostic, and conformance identities with structural redaction.
- [`022-host-neutral-implementation-step-v1.md`](022-host-neutral-implementation-step-v1.md)
  freezes plan-visible implementation execution profiles, prepare atomicity,
  bounded nonblocking steps, port transactions, host operations, and
  equivalent native/message bindings.
- [`023-bounded-deterministic-scheduler-v1.md`](023-bounded-deterministic-scheduler-v1.md)
  freezes atomic runtime preallocation, deterministic round-robin stepping,
  exact queue wakes, staged transactions, cancellation/terminal propagation,
  bounded scheduler evidence, and pool-population reconciliation.
- [`024-structural-flow-v1.md`](024-structural-flow-v1.md) freezes explicit
  coupled/isolated fan-out, deterministic merge policies, bounded structural
  nodes and adapters, plan-v4 identity, and the in-plan fallback boundary.
- [`025-resonance-event-streams-v1.md`](025-resonance-event-streams-v1.md)
  freezes compatible typed event envelopes, plan-v5 retention/provider
  identity, bounded subscription/replay, gaps, crash recovery, and projections.
- [`026-durable-jobs-checkpoints-v1.md`](026-durable-jobs-checkpoints-v1.md)
  freezes plan-v6 finite attempts/leases, Resonance-backed durable progress,
  explicit effect commit and domain acceptance, integrity-protected
  checkpoints, exact resume, and explicit migration.
- [`027-implicit-satisfaction-v1.md`](027-implicit-satisfaction-v1.md) freezes
  language-neutral implicit-satisfaction proofs, complete port/implementation/
  host obligations, deterministic ambiguity policy, and plan-v7 proof identity.
- [`028-bounded-run-stream-v2.md`](028-bounded-run-stream-v2.md) withdraws the
  unreleased v1 run writer and freezes bounded, nonsemantic channel chunks,
  direct structured executor records, version rejection, and exact stream
  failure behavior.
- [`029-runtime-evidence-v1.md`](029-runtime-evidence-v1.md) projects bounded
  executor observations into immutable ExecutionEvent v1 records on the
  shared Resonance fabric with plan-v8 sampling, capacity, path, latency,
  derivation, redaction, and terminal invariants.
- [`030-performance-and-resource-accounting-v1.md`](030-performance-and-resource-accounting-v1.md)
  defines reproducible report metadata, strict deterministic resource/size
  gates, reviewed baselines, and explicit future workload ownership.
- [`031-implementation-artifact-manifests-v1.md`](031-implementation-artifact-manifests-v1.md)
  freezes separate semantic, implementation, and immutable-artifact identities,
  capability-oriented executors, provenance/licensing inspection, and the
  mandatory pre-load integrity/trust boundary.
- [`032-fresh-host-reports-resolution-v1.md`](032-fresh-host-reports-resolution-v1.md)
  freezes fresh canonical host observations, deterministic bounded placement,
  complete candidate rejection, and exact existing-plan sealing without host
  provisioning or mutation.
- [`033-realms-passports-v1.md`](033-realms-passports-v1.md) defines bounded
  realm, entity, key, membership, role, delegation, federation, fresh-status,
  and event-authorship identities without treating any of them as an ambient
  capability or effect grant.
- [`034-browser-host-v1.md`](034-browser-host-v1.md) defines typed browser
  observations, placement boundaries, explicit permission/activation state,
  exact artifact loading, bounded adapter queues, and UI/host separation.
- [`035-patchbay-protocol-v1.md`](035-patchbay-protocol-v1.md) defines
  transport-neutral source, presentation, plan, run, and bounded projection
  resources without turning layout or client state into semantics.
- [`036-exact-compile-package-v1.md`](036-exact-compile-package-v1.md) defines
  explicit exact-plan compilation, distinct input identities, deterministic
  heterogeneous package manifests, pathless envelopes, bounded extraction,
  and non-executing inspection.
- [`037-distributed-cord-v1.md`](037-distributed-cord-v1.md) defines exact
  plan-v9 cross-host bindings, realm-aware live handshakes, finite delivery,
  retry/dedup/reconnect state, carrier-neutral readiness, and correlatable
  transport evidence.
- [`038-embedded-rp2040-v1.md`](038-embedded-rp2040-v1.md) defines the
  allocator-free fixed-storage executor, compact exact-plan binding, linked
  RP2040 firmware budgets, and physical HIL proof boundary.
- [`039-security-boundaries-v1.md`](039-security-boundaries-v1.md) defines
  untrusted-input ceilings, artifact verification/load ownership, bounded
  rejection evidence, honest implementation isolation, and retained
  fuzz/dependency policy automation.
- [`040-zenoh-reference-transport-v1.md`](040-zenoh-reference-transport-v1.md)
  defines plan-v10 exact transport selection, the carrier-neutral backend,
  real hosted Zenoh plaintext/TLS/mTLS proof, deterministic carrier faults,
  and the general firmware-boundary Zenoh-Pico path.

The retrospective
[`C2/C3 integration audit`](../audits/2026-07-29-c2-c3-integration.md)
maps specifications 005–016 to their implementations, persisted schemas,
fixtures, compatibility commitments, and downstream freeze decisions.

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
