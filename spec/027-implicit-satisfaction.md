# Implicit contract satisfaction and structural proof current form

Status: normative current contract

Satisfaction proof schema marker: `0`

ExecutionPlan satisfaction binding schema marker: `0`

## Purpose and boundary

Conduit permits an offered port, implementation, or host capability to satisfy
an explicit required contract without a nominal `implements` declaration at
each authored use site. The omission is only syntactic. A provider still
produces a complete immutable proof over exact operands, and every accepted
non-exact relation used by a runnable plan is retained in plan identity.

This is neither duck typing nor conversion. A proof does not:

- infer meaning from field names, serialized shape, language types, ABI,
  library types, byte length, registry order, or implementation names;
- insert an adapter, codec, unit conversion, clone, declassifier, or migration;
- grant authority, install or provision anything, open a resource, or mutate a
  host report;
- select or mutate an active plan epoch; or
- collapse semantic contract, implementation, artifact, host report, plan,
  evidence, or presentation identities.

Domain facets remain opaque to `conduit-core`. Hosted, native, process, WASM,
and embedded implementations consume the same allocator-free proof contract.

## Directional query and three outcomes

Every proof names a required descriptor and an offered descriptor in that
order, plus one role:

- `port-connection`: offered output to required input;
- `port-substitution`: offered candidate to required boundary;
- `implementation`: offered implementation to required node contract; or
- `host-capability`: offered fresh capability report to required predicate.

The result uses the existing compatibility algebra:

- `compatible` means every required obligation was proven;
- `incompatible` means at least one obligation was disproven; and
- `indeterminate` means a provider, immutable descriptor, fresh report,
  supported facet, or deterministic policy needed to decide is unavailable.

An incompatible result dominates an indeterminate obligation; otherwise any
indeterminate obligation makes the aggregate indeterminate. No unknown fact is
treated as permissive.

## Proof descriptor

`conduit/satisfaction-proof` current schema contains:

- exact role and proof method;
- exact required and offered descriptor kind, schema revision, and semantic
  hash;
- exact provider descriptor and stable provider rule when a provider decides;
- an optional exact deterministic selection-policy descriptor;
- a canonical set of provider-declared structural facets;
- a canonical set of directional obligations;
- aggregate outcome, stable reason, and explanation rule; and
- an optional exact adapter or migration descriptor that is required but not
  applied.

Methods are:

- `exact-nominal`: exact required/offered identity; no provider, facet, or
  expanded obligations are necessary;
- `provider-rule`: an immutable provider and stable rule discharge the complete
  role obligations; and
- `structural-facets`: both operands opt into a complete provider-owned facet
  set and a provider rule discharges the complete role obligations.

Each structural facet contains its stable ID plus exact required-side and
offered-side facet hashes. Each obligation likewise contains its stable ID,
exact required-side fact hash, exact offered-side fact hash, three-valued
outcome, and stable explanation rule. Facet and obligation order is
non-semantic. Duplicate identifiers, missing required obligations, invalid
rules, inconsistent aggregate results, operand mutation, or identity mutation
are rejected.

### Complete port obligations

Non-exact port connection and substitution proofs contain:

`direction`, `semantic-type`, `presence`, `connection-cardinality`,
`value-cardinality`, `delivery`, `temporal`, `terminal`, `sensitivity`,
`authority`, `representation`, `ownership-lifetime`, `flow`, and
`boundedness`.

The exact port hashes and nested type facts remain distinct operands. Equal
record fields do not discharge `semantic-type`. Authority does not excuse
sensitivity mismatch. A representation or ownership mismatch names an
explicit adapter when one exists; its unadapted endpoints remain incompatible.
Every live result still has a finite plan-visible flow allocation.
`validate_port_satisfaction_proof` cross-checks these facts against the current
`PortCompatibilityDecision`; it does not introduce a parallel port evaluator.

### Complete implementation obligations

Implementation proofs contain:

`semantic-contract`, `ports`, `configuration`, `representation`,
`ownership-lifetime`, `lifecycle`, `authority`, `resources`, and
`boundedness`.

The implementation and artifact retain identities distinct from the semantic
contract. The proof does not manufacture a manifest, artifact, resource,
effect, lifecycle guarantee, or execution profile. Issue 25 owns discovery and
manifest encodings; it must supply these exact facts rather than infer from a
language or ABI.

### Complete host obligations

Host-capability proofs contain:

`semantic-capability`, `observation-freshness`, `resources`, `effects`,
`authority`, and `boundedness`.

A Linux report and an RP2040-class report may both prove the same semantic
predicate while retaining distinct provider, implementation, resource, host,
report, and plan identities. A stale report is indeterminate. Issue 26 owns
host report and resolver encodings; a proof itself performs no provisioning or
authority operation.

## Provider and selection contracts

Every hosted type provider exposes an exact immutable provider descriptor.
`TypeSatisfactionReport` retains the existing reasoned type decision, consumer
and producer provider identities, provider rule, and both structural facet
hashes. This is sufficient input for a resolver to construct the portable
proof; it is not a second compatibility algebra.

Candidate selection is deterministic:

- zero compatible candidates yields incompatible unless any candidate is
  indeterminate, in which case it remains indeterminate;
- one compatible candidate needs no selection policy;
- multiple compatible candidates without policy are indeterminate with
  `satisfaction-ambiguous`; and
- with an exact policy, the uniquely lowest policy-owned rank wins; an equal
  best rank remains ambiguous.

Input ordering never breaks a tie. Future resolver policy may define richer
stable ranks, but discovery order, hash-map order, timing, and ambient host
state are forbidden inputs.

## current plan schema

`PlanSatisfactionProof` binds a complete proof to one cord, node
implementation, or node host-capability observation. The plan canonical
identity includes the subject and proof identity.

A current-schema cord whose exact type references differ is runnable only with one
compatible, valid `port-connection` proof whose required port hash equals the
input port hash and whose offered port hash equals the output port hash.
Duplicate bindings, dangling subjects, wrong roles, operand mismatch,
non-compatible proofs, or invalid proof identity fail with `CND-IMP-017`.

Implementation bindings cross-check required semantic contract and offered
implementation descriptor pins. Host bindings cross-check the selected node,
host observation, and offered report hash. Issues 25 and 26 decide when those
bindings are required by their future manifest/report schemas; this
specification defines their earliest owning proof and plan identity.

Every current plan carries its exact satisfaction-proof collection.
Re-resolving the same source to another compatible implementation or host
preserves the source semantic hash but produces a different exact plan hash.

## Stable reasons and diagnostics

Stable proof reasons are:

- `satisfaction-proven`;
- `satisfaction-obligation-rejected`;
- `satisfaction-fact-unavailable`;
- `satisfaction-provider-unavailable`;
- `satisfaction-provider-stale`;
- `satisfaction-host-observation-stale`;
- `satisfaction-unsupported-facet`;
- `satisfaction-ambiguous`;
- `satisfaction-explicit-adapter-required`; and
- `satisfaction-explicit-migration-required`.

Portable plan validation uses `CND-IMP-017`. Provider-specific obligation
reasons remain namespaced and stable.

## Conformance and compatibility

`conformance/c2/implicit-satisfaction.json` freezes structural and nominal
success, same-shape/different-meaning rejection, missing and stale
indeterminacy, deterministic selection and ambiguity, explicit adapters,
semantic boundary violations, Linux/Pico equivalence, order-independent
identity, mutation/omission rejection, source/plan identity separation, and
the authority/provisioning/plan-epoch boundaries.

Existing nominal, opaque, directional, migration, TypeContract, PortContract,
and current plan through current plan contracts retain their current meaning. This
specification operationalizes explicit structural opt-in; it does not turn
structural comparison into an implicit fallback.

## Normative requirements

| ID | Obligation |
|---|---|
| SAT-001 | Retain exact required/offered operands, role, direction, provider rule, outcome, and explanation |
| SAT-002 | Use compatible, incompatible, and indeterminate without permissive unknowns |
| SAT-003 | Require complete role-specific semantic obligations for every non-exact proof |
| SAT-004 | Keep structural facets provider-owned, bilateral, explicit, and domain-open |
| SAT-005 | Never infer satisfaction from shape, language, ABI, library, byte length, or discovery order |
| SAT-006 | Name adapters and migrations without applying them or changing direct compatibility |
| SAT-007 | Grant no authority, provisioning, resource access, or active plan mutation through a proof |
| SAT-008 | Resolve multiple compatible candidates only through exact deterministic policy |
| SAT-009 | Make facet, obligation, registry, manifest, and report ordering non-semantic |
| SAT-010 | Reject duplicate, incomplete, inconsistent, mutated, or wrong-operand proofs |
| SAT-011 | Record every accepted non-exact runnable relation in exact plan identity |
| SAT-012 | Preserve source identity while alternate compatible realizations change plan identity |
| SAT-013 | Keep semantic, implementation, artifact, host observation, plan, evidence, and presentation identities distinct |
| SAT-014 | Preserve current nominal, opaque, migration, and current plan through current plan behavior |
