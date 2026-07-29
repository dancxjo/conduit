# C2/C3 semantic-kernel and authoring integration audit

Issue: [#53](https://github.com/dancxjo/conduit/issues/53)

Audit date: 2026-07-29

Audited baseline: `b65e377009e3cad8f23401f39fd118700efcb37b`

Conformance baseline: `conduit.conformance/v1`, manifest revision 4,
13 suites, 206 normative cases

## Decision

The C2 semantic kernel is internally coherent and remains suitable as the
portable foundation for later implementation and scheduler contracts. The C3
diagnostic boundary is also coherent. No second graph, compatibility,
diagnostic, plan, or evidence model is needed.

The authoring-to-planning boundary is not completely frozen. Two findings need
versioned downstream work:

1. selected-root state currently participates in the authored source hash, and
   the lowered source omits plan-relevant topology and `using` constraints;
   [#64](https://github.com/dancxjo/conduit/issues/64) owns the versioned
   correction;
2. port-group member spans, complete-contract validation, explicit maxima,
   exports, and evidence identity need reconciliation by
   [#44](https://github.com/dancxjo/conduit/issues/44).

These findings do not justify changing the meaning of any frozen v1 artifact.
They do constrain downstream sequencing:

- **#44 is authorized to proceed** as the owning semantic reconciliation
  ticket, subject to the findings below and #64's corrected lowering boundary.
- **#56 is authorized to proceed** by extending, rather than merging, the
  semantic, plan, and evidence identities enumerated here.
- **#20 is blocked from complete implementation and closure** until #56
  supplies its bounded node-step/transaction contract. Deterministic scheduler
  scaffolding against the existing `FlowPolicy` is compatible.
- **#60 remains blocked** on #44, #56, and #20. The existing pool descriptors
  are finite plan inputs, not a complete replicated runtime contract.

## Identity and dependency direction

The audited code preserves this direction:

```text
source bytes
  -> lossless CST
  -> authored source AST
  -> resolved module graph and selected root
  -> lowered semantic topology and source map
  -> exact implementation/host/authority bindings
  -> immutable ExecutionPlan
  -> run identity and immutable ExecutionEvent stream
  -> presentation, logs, projections, and diagnostics
```

Exact content digests, canonical semantic hashes, plan identities, event
identities, and diagnostic records use separate types or versioned domains.
Host observations and revocation status remain observations rather than
semantic descriptor fields. Presentation never feeds a canonical identity.

The one known exception is the selected-root/source-AST boundary recorded as
finding A1 and assigned to #64.

## Integrated issue and artifact map

| Issue / PR | Normative contract | Requirements | Reference implementation | Persisted commitment | Fixture coverage | Audit result |
|---|---|---|---|---|---|---|
| #5 / PR #40 | `spec/005-type-contracts-v1.md` | `TYP-001`–`TYP-012` | `conduit_core::TypeContractRef`; `conduit_runtime::TypeRegistry` | opaque `(contract_id, schema_version, semantic_hash)` reference; provider decisions remain hosted | `conformance/c2/type-contract-v1.tsv`; positive exact/provider, negative malformed, boundary unavailable/unknown, migration directional | coherent; #56 must add representation identity beside, never inside, semantic type identity |
| #6 / PR #43 | `spec/006-port-config-contracts-v1.md` | `PRT-001`–`PRT-008`, `CFG-001`–`CFG-007` | `PortContract`, `ConfigFieldContract`, `ConfigContract`, `config_resolution` | port-contract and config-field schema 1; every port fact hashed; protected values excluded from semantic identity | `port-contract-v1.tsv`, `config-v1.tsv`; positive, negative, cardinality/bounds, default migration | coherent; complete port contracts remain the only group-member boundary |
| #7 / PR #46 | `spec/007-bounded-flow-policy-v1.md` | `FLW-001`–`FLW-012` | `FlowPolicy`, `FlowCapacity`, `FlowWatermarks`, `BoundedFlowQueue` | positive finite item/value/aggregate-byte limits; exact pressure parameters and FIFO blocking | `flow-policy-v1.tsv` plus exhaustive short traces; all policies, invalid bounds, type-proof boundaries, cancellation | coherent and allocator-free; #20 consumes this algebra without adding queue semantics |
| #8 / PR #47 | `spec/008-lifecycle-cancellation-terminal-v1.md` | `LIF-*`, `CAN-*`, `TRM-*`, `REP-*` | lifecycle/cord machines, cancellation registry, terminal resolver | immutable transition vocabulary, explicit causes, bounded cancellation storage, new attempt identity on restart | `lifecycle-v1.tsv`, `terminal-races-v1.tsv`; legal/illegal edges, races, bounds, replicated restart | coherent; #56 maps step outcomes into these states, #60 specializes finite replicated populations |
| #9 / PR #48 | `spec/009-exported-composites-v1.md` | `CMP-001`–`CMP-010` | `CompositeDefinition`, `validate_composite`, hosted `expand_panel` | logical composite and expanded primitive paths remain distinct; exports preserve complete contracts | `composite-v1.tsv`; one-level/nested, recursion, fan-out, bindings, boundary bypass | core/runtime model coherent; corrected C3 lowering must retain this topology (A1/#64) |
| #10 / PR #49 | `spec/010-scoped-authority-v1.md` | `AUT-001`–`AUT-009`, `SEN-001`–`SEN-003` | authority descriptors, deterministic resolver, at-use validation | capabilities, effects, grants, bindings, and revocation observations are separate; protected evidence structural | `authority-v1.tsv`; allow/deny, expiry, delegation, aggregation, redaction | coherent; #56 host operations consume exact bindings and cannot provision |
| #11 / PR #50 | `spec/011-exact-execution-plan-v1.md` | `PLN-001`–`PLN-010` | `ExecutionPlan`, exact leaf hashing, portable validator | schema 1 exact arrangement; immutable source/resolver/host/artifact/node/cord/authority/composite/group/pool references | `execution-plan-v1.tsv`; valid, malformed, order independence, staleness, budget, pool boundary | coherent for current fields; #56 needs a versioned implementation execution-profile reference, #44 must reconcile explicit group maxima |
| #12 / PR #51 | `spec/012-immutable-execution-event-v1.md` | `EVD-001`–`EVD-010` | `ExecutionEvent`, stream validator, owned NDJSON adapter | schema 1 append-only events; exact plan link; separate append/observer/time/causal order; structural redaction | `execution-event-v1.tsv` and `.ndjson`; causation, correction, timestamp inversion, redaction, replay | coherent; transition policy must consume events, not diagnostic prose |
| #13 / PR #52 | `spec/013-conformance-harness-v1.md` | `CNF-001`–`CNF-009` | `conduit-conformance` audit/request/check/reference protocol | manifest revisioning and semantic fixture versions are distinct; request IDs are not semantic identities | all 13 suites; digest/inventory/protocol/property checks | coherent; independent Python canonical verification is now an explicit local/CI gate |
| #14 / PR #54 | `spec/014-panel-grammar-modules-v1.md` | `SRC-001`–`SRC-011` | `conduit-panel` lexer/parser, `SourceDocument`, module resolver | grammar 1, exact UTF-8 content hashes, lossless CST, normalized source-hash domain | `panel-grammar-v1.json`; every production, malformed recovery, imports, roots, groups, pools | grammar is frozen and parse-compatible; selected-root identity leakage requires versioned correction A1/#64 |
| #15 / PR #55 | `spec/015-typed-source-lowering-v1.md` | `LWR-001`–`LWR-012` | `source_lowering`, owned semantic schemas, source maps | distinct hashes for node schemas, lowered config/nodes/group ports/pools/source; protected bindings remain unresolved | `source-lowering-v1.json`; literals, defaults, imports, redaction, groups/pools, errors | config/default lowering is coherent; complete topology and constraints require A1/#64; group details require A2/#44 |
| #16 / PR #58 | `spec/016-structured-diagnostics-v1.md` | `DIA-001`–`DIA-012` | allocator-free `Diagnostic`; owned JSON/fixes/renderer; CLI adapters | diagnostic schema 1, authoritative byte spans, guarded edits, structural redaction | `diagnostics-v1.json`; exact human/ANSI snapshots, JSON, fixes, multi-file, non-UTF-8 | coherent; ownership with #17/#18 confirmed below |

PRs reviewed: #40, #43, #46–#52, #54, #55, and #58. Their merge commits are
recorded in Git history and all referenced source/specification/fixture files
exist at the audited baseline.

## Fixture-class map

The manifest's named coverage representatives are not execution filters; the
reference runs every case. “Malicious/adversarial” below names the cases that
attempt malformed identities, boundary bypass, hidden data, unbounded state,
or protocol ambiguity even where the manifest classifies them under negative
or boundary coverage.

| Issue | Positive | Negative | Boundary | Migration/order | Malicious/adversarial |
|---|---|---|---|---|---|
| #5 | exact nominal and provider acceptance | malformed reference and rejected provider decision | missing provider/unknown strategy | directional provider decision | descriptor identity mismatch and invalid provider decision |
| #6 | complete port acceptance and canonical defaults | direction/type/cardinality/sensitivity failures | committed-to-progressive and optional/default cases | explicit/default equivalence | protected default/semantic identity and implicit adapter rejection |
| #7 | block/reject and every exact policy | zero/inconsistent capacity and forbidden loss | byte/item/watermark/sample limits | discovery-order-independent type facts | attempted hidden overflow/loss and absent coalescer/disposability proof |
| #8 | every legal node/composite/run/cord edge | illegal edge and missing cause | terminal-race permutations and storage limits | composite equivalence/new restart attempt | cancellation cycle, overflow, undersized discard/evidence storage |
| #9 | one-level/nested transparent exports | recursion, dangling/incompatible export/binding | fan-out and nested path flattening | logical/expanded equivalence | direct child-boundary bypass and recursive definitions |
| #10 | exact capability+grant binding | missing/mismatched/expired/revoked grant | delegation, time, aggregation storage | deterministic candidate order | capability-as-permission attempt and protected-value recording leakage |
| #11 | minimal/nested exact plan | dangling, unresolved, stale, over-budget, hash mismatch | bounded pool and scratch limits | collection-order-independent identity | duplicate/dangling facts, partial authority, unbounded/overflow allocation |
| #12 | causal chain and NDJSON replay | malformed payload/reference/sequence | distributed timestamp inversion and redaction | replay/correction equivalence | protected inline data, forged cause, mutation by correction/retraction |
| #13 | complete request/reference/check round trip | malformed manifest/result and unsupported profile | byte/depth/order seeds | revision versus semantic-version rules | digest drift, duplicate/missing/extra request/result identities |
| #14 | every grammar production and resolved import | malformed syntax, duplicate, cycle, bad pin | multiple roots, groups, pools, exact literals | trivia/format equivalence and legacy top level | boundary bypass, path escape, unbounded/overflow group/pool, provisional lowering |
| #15 | all literals/defaults/imports/groups/pools | unknown/missing/mistyped/provider errors | integer limits, protected binding, source origins | map/default/definition format equivalence | nested secret smuggling and protected literal echo attempts |
| #16 | human/ANSI/JSON render and guarded fixes | stale fix and provider indeterminacy | redaction, multi-file, non-UTF-8 bytes | JSON/fix round trip | malformed schema/spans/fixes, overlapping edits, protected argument leakage |

## Persisted field inventory

This inventory is the compatibility surface downstream issues must reference
rather than duplicate:

| Record | Persisted semantic fields |
|---|---|
| `TypeContractRef` | `contract_id`, `schema_version`, `semantic_hash` |
| `PortContract` | `id`, `direction`, `value_type`, `presence`, connection/value cardinality, `delivery`, `temporal`, `terminal`, `sensitivity`, `flow.loss` |
| `ConfigFieldContract` | `key`, exact `value_type`, required/optional/defaulted value, `sensitivity`, `mutability`, semantic/plan `identity` |
| `FlowPolicy` | item/per-value/aggregate-byte capacity, pressure variant and parameters, low/high watermarks, FIFO block fairness |
| lifecycle/cancellation | managed or cord state, stable subject/scope/resource identity, parent scope, finite deadline, drain/abort policy, structured terminal cause and causal parent |
| composite | definition/boundary contract, child IDs/definitions/contracts, bounded internal cords, explicit exports, explicit config bindings, logical instance paths |
| authority | effect/action/requester/audience/resource selector/constraints, observed capability/host/time window, immutable grant/scope/delegation/audit identity, exact resolved binding, separate revocation observation |
| `ExecutionPlan` | schema/identity/source hash, resolver/policy/creation time, aggregate budget, host observations, resources, artifacts, nodes, cords, authorities, composite mappings/exports, groups/members, pools/grants, unresolved selectors |
| `ExecutionEvent` | schema/identity/event/run/plan, append and observer sequences, recorder/observer, logical and expanded paths, kind/detail, named times, correlation, causal/derivation/correction relations, terminality, typed/redacted payload |
| conformance request/result | protocol/fixture version, stable request and fixture IDs, suite/profile/operation/requirements, deterministic environment, exact input/result fields and structured differences |
| source/CST/module | exact source bytes/tokens/spans, grammar version, imports/definitions/nodes/cords/roots/groups/pools, module URI/content hash/import edges; selected root is a known A1 correction |
| lowered source | node path/contract/config/provenance/hash, group member/direction/maximum/port hash/origin, pool policies/bounds/template/hash/origin, source map, aggregate hash; missing topology/constraints are A1 |
| `Diagnostic` | schema/code/severity/message, primary/related spans, public or redacted arguments, notes/help, guarded unapplied fixes, semantic path, causal codes |

## Persisted schema and compatibility commitments

| Identity or record | Version/domain | Compatibility commitment |
|---|---|---|
| canonical descriptor bytes | `conduit.canonical/v1` | byte-for-byte frozen vectors; annotations/default elision do not become semantic |
| type reference | three-field reference version 1 | exact ID, schema revision, and semantic hash; no version-number inference |
| port/config descriptors | schema 1 | all stated fields retain meaning; changes require new descriptor identity/schema-aware migration |
| flow policy | algebra 1 | no sentinel/unbounded capacity or hidden pressure default |
| lifecycle/cancellation | algebra 1 | legal edges, cause precedence, and drain/abort behavior are frozen |
| composite definition | algebra 1 | external access only through complete explicit exports; logical and expanded paths survive |
| authority descriptors | version 1 | observation, permission, binding, and revocation identities remain separate |
| execution plan | schema 1 | immutable exact arrangement; changes to persisted fields require a new schema or explicit compatible extension |
| execution event / NDJSON | schema 1 / representation 1 | append-only, plan-linked, lossless round trip; presentation and logs remain derived |
| conformance protocol | protocol 1, fixture `conduit.conformance/v1` | fixture corrections increment manifest revision; semantic changes require a new fixture/operation version |
| `.panel` source | grammar 1, `conduit.panel-source/v1` | frozen valid syntax/AST meaning; no silent root/default/name-resolution reinterpretation |
| typed lowering | versioned `conduit.* /v1` hash domains | v1 output meaning remains readable; A1 requires a new corrected boundary rather than hash drift |
| structured diagnostic / JSON | schema 1 | codes and fields stable; prose/presentation may improve without changing structured meaning |

## Findings and owners

### A1 — versioned authoring-boundary correction

Classification: **versioned schema correction**

Evidence:

- `Panel.selected_root` contains caller-selected state and
  `semantic_source_hash` includes it.
- `ModuleGraph` separately carries the selected root, so the same fact exists
  at both authored and resolved layers.
- `LoweredSource` retains nodes/configuration, group ports, pools, and source
  maps, but not ordinary cords, composite child/export/binding topology, or
  `Node.constraint`.
- The legacy hosted `Registry::resolve` handles ordinary topology but rejects
  advanced roots/constraints/groups/pools instead of consuming
  `LoweredSource`; it is a strict executable seed, not a replacement plan
  compiler.

Impact: caller root choice can alter authored source identity, while
plan-relevant topology or `using` changes can disappear at the lowering
boundary.

Owner: [#64](https://github.com/dancxjo/conduit/issues/64), coordinated with
#44 and #61. Frozen v1 meaning is retained; the correction must be explicitly
versioned with migration fixtures.

### A2 — port-group reconciliation

Classification: **missing validation/fixture and versioned plan correction if
the explicit maximum cannot be added compatibly**

Evidence:

- keyed members retain only the containing group's `SourceSpan`, not exact
  member spans;
- `OwnedPortReference` carries ID/hash, while the group's separately authored
  direction is not checked against the referenced complete `PortContract`;
- lowered members carry `group_maximum`, but `PlanPortGroup` exposes member
  IDs/ordinals/hashes and only an indirect template hash, not an explicit
  maximum;
- current C3 fixtures count expanded members but do not prove complete
  per-member contracts, nested group exports, order independence, semantic
  maximum changes, or logical/expanded evidence paths.

Impact: grammar-v1 forms are finite, but #44's complete semantic contract is
not yet proven.

Owner: [#44](https://github.com/dancxjo/conduit/issues/44). It must preserve
grammar-v1 meaning and use a versioned plan correction if explicit maximum or
identity fields cannot be added without changing schema 1.

### A3 — implementation execution profile is downstream plan data

Classification: **downstream implementation work**

Evidence: `ResolvedPlanNode` pins semantic contract, implementation manifest,
artifact, host observation, authority, and aggregate allocation, but the
current plan has no distinct representation identity or exact limits for
retained values, per-step scratch, simultaneous leases/reservations, tasks,
pending host operations, or foreign-runtime queues.

Owner: [#56](https://github.com/dancxjo/conduit/issues/56). It may define a
versioned implementation execution-profile descriptor and plan reference. It
must not add language/runtime fields to `PortContract`, `TypeContractRef`, or
semantic node contracts.

### A4 — scheduler sequencing

Classification: **downstream implementation work**

Evidence: the finite `FlowPolicy`, lifecycle, exact cord allocation, plan
budget, and evidence envelopes are sufficient scheduler inputs. The node-side
step, wake-interest, lease/reservation, and false-progress rules are not yet a
normative executable contract.

Owner: [#56](https://github.com/dancxjo/conduit/issues/56) first, then
[#20](https://github.com/dancxjo/conduit/issues/20). #20 must not expose raw
executor queues or invent a Tokio-specific plan route.

### A5 — replicated runtime populations

Classification: **downstream implementation work**

Evidence: grammar/lowering/plan fields provide positive finite live and queued
maxima and worst-case budget hooks. They do not implement admission,
fairness, generation overlap, host-operation bounds, or complete evidence.

Owner: [#60](https://github.com/dancxjo/conduit/issues/60), after #44, #56,
and #20. #57 owns immutable plan-epoch transitions and overlap/rollback
semantics.

### A6 — diagnostic, presentation, and machine-output ownership

Classification: **compatible ownership clarification**

Confirmed boundary:

- #16 owns diagnostic schema 1, diagnostic JSON on stderr, guarded fixes,
  redaction, and the base human renderer.
- #17 owns clap parsing and polished human/status presentation over that exact
  schema.
- #18 owns completions/man pages plus finite result JSON and streaming
  event/evidence NDJSON on stdout.

`--diagnostic-format` cannot be repurposed as result formatting, and generic
result `--format` cannot change diagnostic encoding. Diagnostics may explain
execution decisions but never become `ExecutionEvent` evidence or #57
transition triggers.

Owners: [#17](https://github.com/dancxjo/conduit/issues/17) and
[#18](https://github.com/dancxjo/conduit/issues/18). No corrective schema work
is needed.

### A7 — missing automated audit gates

Classification: **compatible fix in #53**

Before this audit, CI ran the Rust reference and implicit/default run smoke
but did not directly run the independent Python canonical verifier, the
declared Rust 1.85 MSRV, or explicit `conduct --run`. This audit adds those
checks without changing any persisted semantic data.

## Architectural invariant result

| Invariant | Result and evidence |
|---|---|
| `.panel` is authored assemblage, not a runtime species | **Pass.** `conduit-panel` parses source only; core uses ordinary node/composite contracts; runtime expands composites. A1 corrects selected-root placement without adding a runtime panel. |
| composites expose only complete explicit ports and retain provenance | **Pass in C2/runtime; conditional in corrected C3 lowering.** `validate_composite` checks complete port/config semantics and explicit exports. A1/A2 own missing lowering/group provenance. |
| every live cord and implementation-controlled allocation is finite and plan-visible | **Pass for cords; downstream condition for implementations.** `FlowCapacity` and plan queues are finite. A3/#56 must add implementation-controlled bounds before #20 starts nodes. |
| resolution selects/binds without provisioning or ambient authority | **Pass.** catalogs/loaders/observations/grants are explicit inputs; no resolver performs network, filesystem, login, grant acquisition, or artifact fetch. |
| `ExecutionPlan` is exact arrangement data, not source/package/code | **Pass.** plan types pin references and budgets only; package and compile remain separate #61 work. |
| diagnostics do not become evidence or transition triggers | **Pass.** the schemas and streams are separate; A6 fixes downstream ownership. |
| core privileges no language, ABI, async framework, host, transport, or backend | **Pass.** `conduit-core` remains borrowed, allocator-free, `no_std`, and domain-neutral. |
| canonical `conduct [--check\|--explain\|--run] [PANEL\|-]` remains intact | **Pass.** check, explain, explicit run, default run, stdin, diagnostic stderr, and clean value stdout were exercised. |

## Validation record

All commands passed on the final audit change from a clean linked worktree with
no carried repository-local generated artifacts:

```text
just sup
cargo run -p conduit-conformance -- audit conformance/v1/manifest.json
cargo run -p conduit-conformance -- reference conformance/v1/manifest.json
python3 conformance/c1/verify_canonical_v1.py
cargo +1.85.0 check --workspace --all-targets
cargo +1.85.0 check -p conduit-core --no-default-features --target thumbv6m-none-eabi
cargo run -q -p conduct -- --check examples/hello.panel
cargo run -q -p conduct -- --explain examples/hello.panel
cargo run -q -p conduct -- --run examples/hello.panel
cargo run -q -p conduct -- examples/hello.panel
cargo test -p conduit-diagnostics --test diagnostic_vectors
cargo test -p conduct --test diagnostics_cli
```

The reference reported exactly 13 suites and 206 cases. Human and JSON
diagnostic failures wrote no value stdout; human output contained no ANSI
under `--color=never`; JSON stderr decoded as diagnostic schema 1.

Direct inspection additionally covered public types and reference direction in
`conduit-core`, source/CST/module/lowering types in `conduit-panel` and
`conduit-runtime`, hosted diagnostic adapters/rendering, the entire manifest,
all C2/C3 specifications, and the frozen fixture representatives.

## Freeze outcome

The existing C2 schemas and #16 diagnostic schema are frozen at their stated
v1 meanings. Downstream work may extend them only through explicit versioned
descriptors/references and migration rules.

The C3 source/lowering boundary is frozen only as a readable v1 input. It is
not approved as the complete planning boundary until #64 and the relevant #44
findings land. This is a controlled versioned correction, not permission to
reinterpret existing source or fixture identities.
