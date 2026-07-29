# Conformance fixture version 1 history

## Manifest revision 21 — 2026-07-29

- Added the `realms-passports` C2/C4 suite for issue #88.
- Froze separate realm, entity, public-key, credential, role, delegation,
  grant, capability, artifact, transport, federation, status, and event
  authorship identities without admitting private key material.
- Added bounded rotation/replacement, stale/revoked status, attribution,
  directional federation, offline, redaction, constrained-host, and
  resolver-no-mutation vectors.
- Requirement IDs: `RLM-001` through `RLM-024`.
- Migration: browser and host reporters may bind an exact realm/passport
  status result, but neither report collection nor resolution enrolls or
  prompts an entity.

## Manifest revision 20 — 2026-07-29

- Added the `host-resolution` C5 suite for issue #26.
- Froze fresh canonical capability/resource/topology reports, reporter trust,
  deterministic policy/tie behavior, bounded aggregate placement search,
  complete rejection trees, and exact existing-plan sealing.
- Added Linux/RP2040 equivalent-capability and distributed placement fixtures,
  networking/backend facts, and the independent Wi-Fi failure boundaries
  required by issue #85.
- Requirement IDs: `HST-001` through `HST-012` and `RES-001` through
  `RES-026`.
- Migration: expiry triggers explicit re-resolution; no report or resolver
  operation provisions, discovers, configures, or mutates a host.

## Manifest revision 19 — 2026-07-29

- Added the `implementation-artifact-manifests` C5 suite for issue #25.
- Froze separate semantic, implementation, artifact, location, and exact-plan
  identities across native, WASM, FFI, process, firmware, and remote adapters.
- Added mandatory pre-load digest/size checks, explicit target/ABI and trust
  policy outcomes, reproducibility, transitive licensing/provenance references,
  and non-executing inspection.
- Requirement IDs: `MAN-001` through `MAN-010` and `ART-001` through `ART-010`.
- Migration: resolvers may consume v1 manifests; unsupported schemas and
  incomplete security metadata fail closed rather than becoming preferences.

## Manifest revision 18 — 2026-07-29

- Added the `performance-accounting` suite for issue #24.
- Added reviewed report metadata, workload inventory, category-complete
  scheduler allocation/high-water accounting, release artifact size gates,
  and explicit future-workload ownership.
- Shared-runner wall time remains report-only; deterministic resource and
  artifact-growth gates are strict.
- Requirement IDs: `PRF-001` through `PRF-008`.
- Migration: CI must run the checked-in performance gate; benchmarks do not
  become #83 workload or deadline guarantees.

## Manifest revision 17 — 2026-07-29

- Added the `runtime-evidence` suite for issue #23.
- Added plan-v8 exact runtime-evidence mode, shared normative Resonance stream,
  event/byte reserves, deterministic telemetry sampling, and explicit
  sampling summaries while preserving plan-v1 through plan-v7 identities.
- Projected fixed scheduler lifecycle, occupancy, pressure, loss, local
  latency, cancellation, terminal, and derivation observations into immutable
  ExecutionEvent v1 with logical and expanded paths.
- Added direct bounded run-v2 serialization, redaction/value-byte separation,
  distributed causation boundaries, and fail-closed required/summary/terminal
  cases.
- Requirement IDs: `RTE-001` through `RTE-016`.
- Migration: re-lower old plans into schema 8 with an explicit disabled or
  recording policy; semantic value/clock work remains #80 and deadline
  guarantees remain #83.

## Manifest revision 16 — 2026-07-29

- Added the `conduct-run-stream` suite for issue #72.
- Preserved the pre-release `conduit.run/v1` fixture while explicitly
  withdrawing its writer and selecting bounded `conduit.run/v2`.
- Froze 4,096-byte nonsemantic channel chunks, finite encoded/serialized
  ceilings, exact per-channel reconstruction, global adapter sequence, and
  checked arithmetic.
- Added first/later broken-pipe and non-broken partial-failure cases, strict
  version rejection, clean machine output, and a direct bounded
  `ExecutionEvent` path that never infers evidence from channel bytes.
- Requirement IDs: `RUN-001` through `RUN-014`.
- Migration: v1 remains historical evidence only; consumers must select v2
  and must not reinterpret `channel_chunk` as a typed value or event.

## Manifest revision 15 — 2026-07-29

- Added the `implicit-satisfaction` suite for issue #84.
- Froze language-neutral exact/provider/structural proofs with complete
  port, implementation, and host obligations and three-valued outcomes.
- Added deterministic candidate selection, explicit adapter-only behavior,
  missing/stale provider and host handling, order-independent proof identity,
  and mutation/omission rejection.
- Added plan-v7 satisfaction-proof bindings while preserving source identity
  across alternate compatible realizations.
- Requirement IDs: `SAT-001` through `SAT-014`.
- Migration: plan-v1 through plan-v6 identities remain unchanged and contain
  no satisfaction-proof facts.

## Manifest revision 14 — 2026-07-29

- Added the `durable-job` suite for issue #22.
- Froze plan-v6 durable job/provider/allocation identity, distinct attempts,
  leases and acceptance, finite retry/cancel/checkpoint budgets, truthful
  delivery and effect commits, exact resume, and explicit migration.
- Added independent append/checkpoint crash boundaries, corrupt/incompatible
  checkpoint, source offset, queued value, lease expiry, terminal replay,
  domain validation, and non-checkpointable restart coverage.
- Requirement IDs: `JOB-001` through `JOB-012`.
- Migration: durable jobs and checkpoints require plan v6; plan-v1 through
  plan-v5 identities remain unchanged and contain no durable-job facts.

## Manifest revision 13 — 2026-07-29

- Added the `resonance` suite for issue #79.
- Extended frozen ExecutionEvent v1 compatibly into typed evidence, domain,
  and control streams while keeping live cord values and projections distinct.
- Added plan-v5 provider/retention/allocation identity, bounded coupled and
  isolated subscribers, explicit gaps/replay, append crash recovery,
  redaction, correction, embedded-retained, and projection fixtures.
- Requirement IDs: `RSN-001` through `RSN-016`.
- Migration: event-v1 and plan-v1 through plan-v4 identities remain unchanged;
  explicit event streams are re-lowered into plan v5.

## Manifest revision 12 — 2026-07-29

- Added the `structural-flow` suite for issue #21.
- Froze plan-v4 coupled and isolated fan-out, deterministic merge selection,
  bounded structural nodes, explicit adapters, terminal/cancellation cases,
  and the separation between in-plan fallback and plan transitions.
- Added reference coverage for atomic publication under a slow branch,
  priority starvation, event-time lateness, and rejection of implicit or
  unauthorized duplication.
- Requirement IDs: `STR-001` through `STR-014`.
- Migration: plan v1-v3 identities remain readable and unchanged; structural
  topology is explicitly re-lowered into plan v4.

## Manifest revision 10 — 2026-07-29

- Added the `implementation-step` suite and language-neutral
  `conformance/c4/implementation-step-v1.json` fixture for issue #56.
- Added canonical execution-profile schema 1 and ExecutionPlan schema 3
  pinning without changing frozen plan-v1/v2 identity.
- Froze prepare-all atomicity, bounded nonblocking step outcomes, exact wake
  interests, executor-mediated port transactions, host-operation bindings,
  optional checkpointing, and executor-owned observations.
- Added equivalent direct-native and versioned process/WASM-style message
  bindings while keeping the normative contract ABI/framework neutral.
- Requirement IDs: `IMP-001` through `IMP-016`.
- Migration: plan v1/v2 remain readable and unchanged; a runnable v3 plan
  requires one exact execution profile per primitive.

## Manifest revision 9 — 2026-07-29

- Added the `inspection` suite and language-neutral
  `conformance/c3/inspection-v1.json` fixture for issue #19.
- Added bounded marker-only inspection for panel source, typed lowerings and
  plans, execution evidence, structured diagnostics, and conformance data.
- Froze structural redaction, local-reference confinement, no-execution
  behavior, finite CLI streams, generated assets, and output-failure policy.
- Requirement IDs: `INSP-001` through `INSP-015`.
- Migration: the additive `conduct inspect` secondary operation preserves the
  canonical run/check/explain path and rejects ad-hoc plan/lowering encodings
  until an owning standalone codec is specified.

## Manifest revision 8 — 2026-07-29

- Added the `conduct-output` suite and language-neutral
  `conformance/c3/conduct-output-v1.json` fixture for issue #18.
- Added versioned finite check/explain JSON, ordered lossless run NDJSON,
  independent result/diagnostic format matrices, bounded progress, quiet and
  verbosity policy, generated CLI assets, and output/pipe boundaries.
- Preserved diagnostic schema v1, human invocation defaults, and immutable
  execution-evidence identity; the run summary is not relabeled as evidence.
- Requirement IDs: `OUT-001` through `OUT-012`.
- Migration: existing human invocations are unchanged. Consumers opt into
  `conduit.result/v1` or `conduit.run/v1`; unsupported operation/format pairs
  are rejected rather than silently falling back.

## Manifest revision 7 — 2026-07-29

- Added the `source-lowering-v2` suite and language-neutral
  `conformance/c3/source-lowering-v2.json` fixture for issue #64.
- Preserved source/lowering v1 identities while separating caller-selected
  root state from authored source-v2 identity.
- Added complete cord, composite-child, export, binding, unresolved-constraint,
  source-map, version-selection, and verified migration cases.
- Requirement IDs: `SL2-001` through `SL2-012`.
- Migration: a persisted v1 lowering requires its exact resolved source graph
  for verified v2 re-lowering; runners must reject unsupported versions rather
  than fall back.

## Manifest revision 6 — 2026-07-29

- Added the `conduct-cli` suite and language-neutral
  `conformance/c3/conduct-cli-v1.json` command and presentation cases for
  issue #17.
- Preserved the canonical no-subcommand invocation and structured diagnostic
  flags while freezing stream ownership, TTY/environment color precedence,
  bounded status, broken-pipe, closed-stderr, and input/output failure
  behavior.
- Recorded help and argument snapshots plus release startup and binary-size
  measurements; result/evidence formats and progress machinery remain owned by
  issue #18.
- Requirement IDs: `CLI-001` through `CLI-012`.
- Migration: runners supporting `conduit.c3` but not these hosted CLI
  operations must report them as unsupported rather than silently skipping
  them.

## Manifest revision 5 — 2026-07-29

- Added the `port-groups-correlation` suite and the language-neutral
  `conformance/c2/port-group-correlation-v1.json` fixture for issue #44.
- Added explicit plan-schema-1 preservation and plan-schema-2 migration cases;
  no schema-1 expected identity or meaning was replaced.
- Added keyed/indexed member identity, exact span, complete-contract,
  maximum, export, order-independence, correlation-family, propagation,
  retry/resume, generation/epoch, and forbidden allocator cases.
- Requirement IDs: `PGC-001` through `PGC-010` and `COR-001` through
  `COR-007`.
- Migration: runners supporting `conduit.c3` but not these operations must
  report them as unsupported rather than silently skipping them.

## Manifest revision 4 — 2026-07-28

- Added the independent `diagnostics` suite and
  `conformance/c3/diagnostics-v1.json` cases for issue #16.
- Existing version 1 requests and expected outputs are unchanged.
- Expected version: new `structured-diagnostic-v1` operation; no previous
  operation result was replaced.
- Requirement IDs: `DIA-001` through `DIA-012`.
- Migration: runners supporting `conduit.c3` but not structured diagnostics
  must report this operation as unsupported rather than silently skipping it.

## Manifest revision 3 — 2026-07-28

- Added the independent `source-lowering` suite and
  `conformance/c3/source-lowering-v1.json` cases for issue #15.
- Extended the existing panel grammar artifact with typed-literal acceptance
  and malformed exact-decimal cases required by source lowering.
- Existing version 1 requests and expected outputs are unchanged.
- Expected version: new `source-lowering-v1` operation; no previous operation
  result was replaced.
- Requirement IDs: `LWR-001` through `LWR-012`.
- Migration: runners supporting `conduit.c3` but not typed lowering must report
  this operation as unsupported rather than silently skipping its cases.

## Manifest revision 2 — 2026-07-28

- Added the independent `panel-source` suite and
  `conformance/c3/panel-grammar-v1.json` cases for issue #14.
- Existing version 1 requests and expected outputs are unchanged.
- Expected version: new `panel-source-v1` operation at grammar version 1; no
  previous operation result was replaced.
- Requirement IDs: `SRC-001` through `SRC-011`.
- Migration: runners may add the `conduit.c3` profile explicitly; they must
  report it as unsupported rather than silently skipping its cases.

## Manifest revision 1 — 2026-07-28

- Established `conduit.conformance/v1` and protocol version 1.
- Indexed the reviewed canonical, compatibility, type, port/config, flow,
  lifecycle, composite, authority, plan, and evidence artifacts from issues
  #3 through #12.
- Added deterministic byte, recursion, and discovery-order seeds.
- Expected version: initial version; no prior expected output or migration.
- Requirement IDs: `CNF-001` through `CNF-009`, plus the semantic requirement
  IDs recorded per suite in `manifest.json`.
- This is an initial semantic fixture version, not a correction.
