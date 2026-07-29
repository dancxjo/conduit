# Conformance fixture version 1 history

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
