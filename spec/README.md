# Conduit specifications

The specifications distinguish facts that are easy to conflate:

1. semantic contracts;
2. concrete implementations;
3. host capability and resource observations;
4. editable `.panel` source;
5. exact resolved execution plans;
6. immutable execution evidence;
7. Patchbay presentation and mutable product projections.

The current documents are candidates, not a claim of ecosystem stability,
security support, formal verification, certification, or fitness for
consequential deployment. Specifications 041–045 define candidate containment
contracts; they do not by themselves establish that every implementation,
provider, distribution, or composition enforces those contracts.

Read [Safety, deployment boundaries, and stewardship](../docs/safety-and-stewardship.md)
and [SECURITY.md](../SECURITY.md) alongside this inventory. The cross-cutting
containment program is tracked by
[#92](https://github.com/dancxjo/conduit/issues/92).

- [`000-c0-evidence.md`](000-c0-evidence.md) records the brownfield evidence
  that justifies extracting Conduit.
- [`001-c1-bootstrap-meta.md`](001-c1-bootstrap-meta.md) defines how later
  specifications and descriptors become normative and conformant.
- [`002-roadmap.md`](002-roadmap.md) defines the smallest proof sequence from
  the current one-shot executor to embedded and domain integrations.
- [`003-canonical-descriptor.md`](003-canonical-descriptor.md) freezes the
  allocator-free canonical descriptor bytes and semantic-hash vectors.
- [`004-directional-compatibility.md`](004-directional-compatibility.md)
  freezes reasoned compatibility queries, version direction, substitution, and
  migration identity.
- [`005-type-contracts.md`](005-type-contracts.md) defines opaque
  domain-owned type references, hosted provider discovery, and explicit
  nominal, structural, and opaque comparison.
- [`006-port-config-contracts.md`](006-port-config-contracts.md) freezes
  complete port boundaries, directional diagnostics, and separate typed
  configuration/default/redaction semantics.
- [`007-bounded-flow-policy.md`](007-bounded-flow-policy.md) freezes
  finite item/byte capacity, pressure transitions, type-gated loss, evidence,
  and the allocator-free reference queue.
- [`008-lifecycle-cancellation-terminal.md`](008-lifecycle-cancellation-terminal.md)
  freezes lifecycle transitions, bounded hierarchical cancellation,
  deterministic terminal races, and replicated-child supervision.
- [`009-exported-composites.md`](009-exported-composites.md) freezes
  ordinary composite definitions, transparent exports, parameter bindings,
  recursive lowering, and logical/expanded provenance.
- [`010-scoped-authority.md`](010-scoped-authority.md) separates fresh
  host capability, effects, grants, exact bindings, delegation, revocation,
  and structurally redacted sensitivity handling.
- [`011-exact-execution-plan.md`](011-exact-execution-plan.md) freezes
  exact runnable-plan contents, canonical identity, portable validation,
  freshness, resource accounting, and bounded expansion/pool identity.
- [`012-immutable-execution-event.md`](012-immutable-execution-event.md)
  freezes append-only run evidence, causation/correlation/time separation,
  structural redaction, replay validation, and hosted NDJSON round-tripping.
- [`013-conformance-harness.md`](013-conformance-harness.md) freezes the
  language-neutral fixture inventory, NDJSON runner protocol, structured
  comparison results, deterministic seeds, and fixture review/version rules.
- [`014-panel-grammar-modules.md`](014-panel-grammar-modules.md) freezes
  `.panel` current grammar, lossless CST/source-AST separation, deterministic
  imports and roots, and bounded group/pool authoring forms.
- [`015-typed-source-lowering.md`](015-typed-source-lowering.md) freezes
  exact typed literals, schema validation and defaults, provenance-safe
  semantic lowering, cross-module source maps, and finite group/pool
  expansion.
- [`016-structured-diagnostics.md`](016-structured-diagnostics.md)
  freezes allocator-free diagnostic data, exact source spans, structural
  redaction, guarded fixes, lossless JSON, and hosted terminal rendering.
- [`017-port-groups-correlation.md`](017-port-groups-correlation.md)
  reconciles finite keyed/indexed port groups through source, lowering,
  current plan, exports, and evidence, and freezes distinct correlation identity
  allocators, scopes, lifetimes, and propagation.
- [`018-conduct-cli.md`](018-conduct-cli.md) freezes the canonical
  `conduct` command model, structured failure routing, stdout/stderr ownership,
  terminal and color policy, broken-pipe behavior, and measured dependency
  cost.
- [`019-source-lowering.md`](019-source-lowering.md) defines current
  identities while moving root selection out of authored source identity and
  retaining complete cords, composite relationships, constraints, and exact
  provenance in corrected current lowering.
- [`020-conduct-output.md`](020-conduct-output.md) freezes reproducible
  completions and manuals, independent result and diagnostic selectors,
  finite result JSON, ordered run NDJSON, quiet/verbosity, bounded progress,
  and machine-safe stream behavior.
- [`021-safe-inspection.md`](021-safe-inspection.md) freezes bounded,
  marker-only, non-executing inspection across source, lowering, plan,
  evidence, diagnostic, and conformance identities with structural redaction.
- [`022-host-neutral-implementation-step.md`](022-host-neutral-implementation-step.md)
  freezes plan-visible implementation execution profiles, prepare atomicity,
  bounded nonblocking steps, port transactions, host operations, and
  equivalent native/message bindings.
- [`023-bounded-deterministic-scheduler.md`](023-bounded-deterministic-scheduler.md)
  freezes atomic runtime preallocation, deterministic round-robin stepping,
  exact queue wakes, staged transactions, cancellation/terminal propagation,
  bounded scheduler evidence, and pool-population reconciliation.
- [`072-persistent-exact-run-session.md`](072-persistent-exact-run-session.md)
  freezes explicit Start, owned exact runtime snapshots, bounded cooperative
  pumping, Waiting/Quiescing/Aborting/Terminal distinction, exact wakes,
  bounded provider cleanup, and terminal-only release.
- [`024-structural-flow.md`](024-structural-flow.md) freezes explicit
  coupled/isolated fan-out, deterministic merge policies, bounded structural
  nodes and adapters, current plan identity, and the in-plan fallback boundary.
- [`025-resonance-event-streams.md`](025-resonance-event-streams.md)
  freezes compatible typed event envelopes, current plan retention/provider
  identity, bounded subscription/replay, gaps, crash recovery, and projections.
- [`026-durable-jobs-checkpoints.md`](026-durable-jobs-checkpoints.md)
  freezes current plan finite attempts/leases, Resonance-backed durable progress,
  explicit effect commit and domain acceptance, integrity-protected
  checkpoints, exact resume, and explicit migration.
- [`027-implicit-satisfaction.md`](027-implicit-satisfaction.md) freezes
  language-neutral implicit-satisfaction proofs, complete port/implementation/
  host obligations, deterministic ambiguity policy, and current plan proof identity.
- [`028-bounded-run-stream.md`](028-bounded-run-stream.md) withdraws the
  unreleased current run writer and freezes bounded, nonsemantic channel chunks,
  direct structured executor records, version rejection, and exact stream
  failure behavior.
- [`029-runtime-evidence.md`](029-runtime-evidence.md) projects bounded
  executor observations into immutable ExecutionEvent current records on the
  shared Resonance fabric with current plan sampling, capacity, path, latency,
  derivation, redaction, and terminal invariants.
- [`030-performance-and-resource-accounting.md`](030-performance-and-resource-accounting.md)
  defines reproducible report metadata, strict deterministic resource/size
  gates, reviewed baselines, and explicit future workload ownership.
- [`031-implementation-artifact-manifests.md`](031-implementation-artifact-manifests.md)
  freezes separate semantic, implementation, and immutable-artifact identities,
  capability-oriented executors, provenance/licensing inspection, and the
  mandatory pre-load integrity/trust boundary.
- [`032-fresh-host-reports-resolution.md`](032-fresh-host-reports-resolution.md)
  freezes fresh canonical host observations, deterministic bounded placement,
  complete candidate rejection, and exact existing-plan sealing without host
  provisioning or mutation.
- [`033-realms-passports.md`](033-realms-passports.md) defines bounded
  realm, entity, key, membership, role, delegation, federation, fresh-status,
  and event-authorship identities without treating any of them as an ambient
  capability or effect grant.
- [`034-browser-host.md`](034-browser-host.md) defines typed browser
  observations, placement boundaries, explicit permission/activation state,
  exact artifact loading, bounded adapter queues, and UI/host separation.
- [`035-patchbay-protocol.md`](035-patchbay-protocol.md) defines
  transport-neutral source, presentation, plan, run, and bounded projection
  resources without turning layout or client state into semantics.
- [`036-exact-compile-package.md`](036-exact-compile-package.md) defines
  explicit exact-plan compilation, distinct input identities, deterministic
  heterogeneous package manifests, pathless envelopes, bounded extraction,
  and non-executing inspection.
- [`037-distributed-cord.md`](037-distributed-cord.md) defines exact
  current plan cross-host bindings, realm-aware live handshakes, finite delivery,
  retry/dedup/reconnect state, carrier-neutral readiness, and correlatable
  transport evidence.
- [`038-embedded-rp2040.md`](038-embedded-rp2040.md) defines the
  allocator-free fixed-storage executor, compact exact-plan binding, linked
  RP2040 firmware budgets, and physical HIL proof boundary.
- [`039-security-boundaries.md`](039-security-boundaries.md) defines
  untrusted-input ceilings, artifact verification/load ownership, bounded
  rejection evidence, honest implementation isolation, and retained
  fuzz/dependency policy automation.
- [`040-zenoh-reference-transport.md`](040-zenoh-reference-transport.md)
  defines current plan exact transport selection, the carrier-neutral backend,
  real hosted Zenoh plaintext/TLS/mTLS proof, deterministic carrier faults,
  and the general firmware-boundary Zenoh-Pico path.
- [`041-administrative-containment.md`](041-administrative-containment.md)
  defines current plan domain-owned administrative effect classes, distinct
  proposal/approval/commit/execution identities, independent threshold
  approval, monotonic delegation/recovery, and protected ceremonies.
- [`042-persistent-policy-budgets.md`](042-persistent-policy-budgets.md)
  defines current plan host/site/realm anchored current, rolling, lifetime, and
  finite-lease governance budgets with atomic idempotent reservations,
  durable recovery, fresh status, and independent approval for increases.
- [`043-hazardous-effect-closure.md`](043-hazardous-effect-closure.md)
  defines current plan domain-owned effect classes, bounded whole-plan and
  transition closure, exact toxic combinations and declared propagation,
  independent expiring permits, and secret-safe proof trees.
- [`044-safe-genesis-and-distribution.md`](044-safe-genesis-and-distribution.md)
  defines isolated realm genesis, local-only bootstrap into quarantine,
  bounded deliberately-public operations, safe hosted/browser/constrained
  distribution defaults, exact provider opt-in, and monotonic recovery.
- [`045-independent-inhibit-plane.md`](045-independent-inhibit-plane.md)
  defines current plan hazardous-host bindings, fresh independent inhibit
  observations, finite command leases and domain-owned envelopes, local
  fail-safe transitions, retained latches, and separately approved clear.
- [`046-standard-node-library.md`](046-standard-node-library.md) defines
  bounded standard node families, non-escalating retry, and narrow exact-bound
  host-service shapes while leaving safety-program policy seams unresolved.
- [`047-adversarial-containment-conformance.md`](047-adversarial-containment-conformance.md)
  defines deterministic valid-composition attacks, per-step global
  containment checks, reproducible campaigns, and honest hosted,
  constrained, and physical-HIL claim boundaries.
- [`048-http-serving-profile.md`](048-http-serving-profile.md) defines
  domain-owned HTTP types and ordinary serving composites, exact bounded host
  selection, explicit plaintext/direct-TLS/trusted-proxy modes, deterministic
  routing and session behavior, and real Linux TCP/rustls proof.
- [`049-typed-supervision.md`](049-typed-supervision.md) defines the
  allocator-free terminal-observation and finite decision contract, explicit
  current grammar bindings, current lowered source, exact current plan accounting,
  deterministic nesting/races/failure propagation, and hosted, browser, and
  constrained witnesses.
- [`050-replicated-composite-pools.md`](050-replicated-composite-pools.md)
  defines current-schema exact pool admission, deterministic instance and attempt
  identity, atomic fixed reservations, supervision, cleanup, causal evidence,
  and old/candidate/rollback generation overlap.
- [`051-live-plan-transitions.md`](051-live-plan-transitions.md) defines
  immutable plan epochs, exact cold/quiescent/stateful replacement,
  independently admitted prepare/barrier/drain/state/replay/rebind/commit/
  rollback, persistent containment and inhibit facts, and opaque Tongues plus
  concrete HTTP generation witnesses.
- [`052-node-interface-contract.md`](052-node-interface-contract.md)
  defines allocator-free named multi-port node interfaces, closed optional
  member semantics, exact non-port requirements, and reasoned directional
  satisfaction shared by primitive and composite-derived node contracts.
- [`053-panel-interface-syntax.md`](053-panel-interface-syntax.md)
  defines Panel interface declarations and implements claims, corrected
  semantic source identity, typed lowering, diagnostics, and exact
  primitive/composite satisfaction.
- [`054-text-format.md`](054-text-format.md) defines the final typed
  template-plus-values formatter, exact text/integer/format-values
  descriptors, finite placeholder grammar, normalized failures, provider
  separation, migration, exact execution, and Patchbay proof.
- [`055-value-envelope-clock-feedback.md`](055-value-envelope-clock-feedback.md)
  defines plan-authorized bounded value metadata, exact clock conversion with
  uncertainty, and finite delay/state feedback admission without introducing
  a second event model or scheduler.
- [`056-resource-lease-effect-commit.md`](056-resource-lease-effect-commit.md)
  defines finite run-scoped resource leases, domain-owned commit boundaries,
  honest retry semantics, bounded cleanup, and deterministic plus real Linux
  effect witnesses.
- [`057-workload-admission-deadline.md`](057-workload-admission-deadline.md)
  separates finite workload reservations and exact deadline enforcement from
  measurements, benchmarks, and best-effort host observations.
- [`058-cross-host-provider-conformance.md`](058-cross-host-provider-conformance.md)
  proves optional providers and namespaced extensions across Linux,
  browser/WASM, constrained, deterministic, and describe-only hosts while
  retaining exact satisfaction, conformance, artifact, observation, adapter,
  and binding identities.
- [`059-library-catalog-inventory.md`](059-library-catalog-inventory.md)
  checks every published node's classification, package owner, exact
  descriptor, provider separation, fixture, migration, and Tour ownership,
  and generates the shared documentation/Patchbay index.
- [`060-text-lines-join.md`](060-text-lines-join.md) freezes bounded,
  chunk-independent logical line splitting and finite ordered text joining,
  including exact state, overflow, cancellation, and provider boundaries.
- [`061-panel-directional-syntax.md`](061-panel-directional-syntax.md)
  freezes current grammar logographic directional declarations, equivalent input
  spellings, canonical migration, explicit cord endpoints, and source-AST
  current-schema identity.
- [`061-bounded-filesystem.md`](061-bounded-filesystem.md) defines opaque
  resource handles and finite read, write, and watch boundaries without
  ambient path semantics.
- [`062-evictable-blob-cache.md`](062-evictable-blob-cache.md) defines the
  optional bounded best-effort cache, provider/run-scoped handles, integrity,
  retention, and explicit eviction outcomes.
- [`063-bounded-process-exec.md`](063-bounded-process-exec.md) defines the
  single optional three-stream exec boundary, literal command identity,
  independent output pressure, and finite termination cleanup.
- [`064-bounded-application-sockets.md`](064-bounded-application-sockets.md)
  defines four exact bounded TCP and UDP application socket operations without
  ambient DNS, configuration, firewall, TLS, or HTTP authority.
- [`065-bounded-http-client.md`](065-bounded-http-client.md) defines one
  bounded outbound HTTP/HTTPS request operation with exact network, authority,
  TLS, redirect, proxy, cancellation, and terminal boundaries.
- [`066-bounded-media-values.md`](066-bounded-media-values.md) defines finite
  host-neutral media time, stream, audio/image frame, packet, metadata, and
  exact compatibility boundaries without codecs or devices.
- [`067-panel-capsules.md`](067-panel-capsules.md) defines the bounded readable
  authored-program capsule, lossless source projection, exact artifact and
  sensitivity policy, deterministic offline tooling, and separation from
  plans, live epochs, site bindings, presentation, and evidence.
- [`068-bounded-media-codecs.md`](068-bounded-media-codecs.md) separates exact
  probe, demux, mux, decode, and encode providers from media values and proves
  one bounded content-addressed PCM/WAVE profile.
- [`069-bounded-learned-inference.md`](069-bounded-learned-inference.md) defines
  distinct content-addressed model, schema, runtime, device, resource, and
  provider identities with one finite deterministic inference proof.
- [`070-bounded-spatial-foundation.md`](070-bounded-spatial-foundation.md)
  defines explicit bounded frames, transforms, stamped values, clock
  conversion, uncertainty, calibration, lookup, interpolation, and projection
  without an ambient world or framework identity.
- [`075-bounded-audio-processing.md`](075-bounded-audio-processing.md) defines
  exact bounded PCM mix, gain/ramp, named channel matrices, resample,
  trim/fade, meter side output, and their standing clock/control composition.
- [`076-standing-clocked-signals.md`](076-standing-clocked-signals.md) defines
  distinct event, gate, control, audio, and retained-state ports; clocked
  modulation, sequencing, mixing, feedback, Waiting, observation, and exact
  lifecycle semantics for standing patches.
- [`077-standing-network-services.md`](077-standing-network-services.md)
  defines exact link, frame, packet, datagram, stream, session, control, and
  retained-state values; standing services, finite routes and sessions,
  independent effects, observation, provider plurality, and explicit stop.
- [`078-audio-device-boundaries.md`](078-audio-device-boundaries.md) defines
  observed capture and playback resources, exact negotiation, authority,
  clocks, hosted and virtual providers, standing lifecycle, and failures.
- [`079-temporal-modalities.md`](079-temporal-modalities.md) defines exact
  ordinary-value, closing-flow, open-flow, and current-observation contracts,
  compatibility, identity, and explicit conversion boundaries.
- [`071-bounded-brainstem-network.md`](071-bounded-brainstem-network.md)
  separates AP, DHCP, ICMP, DNS-SD, transport/application protocols,
  observation, and Netherwick robot authority with finite no-radio fixtures.

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
