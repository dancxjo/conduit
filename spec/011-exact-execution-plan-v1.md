# Exact ExecutionPlan identity and portable validation version 1

Status: stable

ExecutionPlan schema version: 1

This document remains the frozen schema-1 contract. Specification
[`017-port-groups-correlation-v1.md`](017-port-groups-correlation-v1.md)
defines the compatible reader behavior and the explicit port-group correction
in ExecutionPlan schema 2; it does not change any schema-1 identity.
Specification
[`022-host-neutral-implementation-step-v1.md`](022-host-neutral-implementation-step-v1.md)
adds one exact implementation execution profile per primitive in plan schema
3 without changing schema-1 or schema-2 identity.

## Identity boundary

An `ExecutionPlan` is one exact runnable arrangement. It is distinct from:

- `.panel` source text and source locations;
- semantic node, port, type, effect, and composite contracts;
- implementation and artifact descriptors;
- mutable host observations;
- run evidence and resumable state; and
- Patchbay layout, selection, labels, and other presentation.

The plan references and pins those inputs; it does not become any of them.
Equivalent source formatting or presentation produces the same plan identity
when every semantic input, observation, resolution choice, and bound remains
the same. A change to source meaning, resolver policy, implementation,
artifact, host report, resource allocation, queue policy, authority, expansion,
or pool maximum changes the identity.

`PlanGraph` is the earlier semantic topology used for node/cord compatibility
before implementation resolution. It is not an `ExecutionPlan` and cannot be
presented to the exact runnable-plan validator.

The plan contains no handler code. It is not source, universal bytecode, ELF,
a container image, a native executable, or a promise that an arbitrary host
can execute it.

## Exact schema

Version 1 contains:

- the supported plan schema version and plan semantic hash;
- source semantic hash, excluding source bytes and presentation;
- resolver descriptor ID, version, and semantic hash plus policy hash;
- the named time basis and tick at which resolution completed;
- an aggregate resource ceiling;
- immutable host-report IDs, semantic hashes, hosts, and freshness windows;
- content-addressed artifact IDs and SHA-256 content digests;
- exact primitive nodes;
- exact bounded cords;
- required effects and resolved authority;
- logical-to-expanded composite and export mappings;
- port-group template and derived-member identities;
- bounded replicated-composite pool reservations; and
- any unresolved selector retained by a draft.

A runnable v1 plan has an empty unresolved-selector collection. The schema can
represent an unresolved draft for explanation, but portable runnable
validation rejects it before any node starts.

### Nodes

Each `ResolvedPlanNode` pins:

- expanded `InstancePath`;
- semantic contract ID, schema version, and hash;
- implementation-manifest ID, schema version, and hash;
- lifecycle-policy ID, schema version, and hash;
- artifact ID;
- host-report ID and concrete host;
- exact resource allocation;
- the set of required concrete resource-binding IDs; and
- the set of required effect hashes.

Every required resource ID resolves to one `PlanResourceBinding` containing
the exact resource kind/ID and host report. Every required effect hash has
exactly one matching `PlanAuthority` entry for that node. Resource or authority
entries not named by the node are also invalid. These bidirectional rules
prevent omission from being interpreted as “no resource or grant required.”

### Cords

Each `ResolvedPlanCord` pins stable cord ID, output and input paths/port IDs,
port-contract hashes, exact type references, full `FlowPolicy`, and accounted
queue memory. The source endpoint is structurally output and the destination
is structurally input. Type references match exactly. Queue memory equals the
finite aggregate byte capacity; there is no sentinel for unbounded capacity.

### Host observations and artifacts

A node's artifact and host-report IDs resolve within the plan. Host identity
must match the selected node host. A host report has a named monotonic time
basis, inclusive observation tick, and exclusive validity end. It is fresh
both at plan creation and at run start.

Artifact content digests are distinct types from descriptor semantic hashes.
The plan does not store a download URL or claim to provision a missing
artifact.

### Authority

`PlanAuthority` retains the full effect, capability observation, immutable
grant, their canonical hashes, and exact `ResolvedAuthorityBinding`. Portable
validation rechecks:

- requesting node path and required-effect membership;
- effect and grant hashes;
- selected host, action, resource, grant, audit identity, and time basis;
- capability freshness; and
- grant scope, audience, constraints, delegation, and expiry.

Validation occurs at plan creation and again at run start. Live revocation
observations remain outside immutable plan identity and are checked by the
authority use boundary from specification 010.

### Resource budget

`PlanResourceBudget` independently bounds memory bytes, storage bytes, CPU
units, timers, transports, checkpoints, and evidence bytes. Portable
validation uses checked arithmetic and rejects overflow.

The aggregate charge is:

```text
sum(fixed node allocations)
+ sum(cord aggregate queue bytes as memory)
+ sum(instance-pool worst-case reservations)
```

Every component must fit the plan ceiling. Executors may enforce tighter
host-specific limits, but cannot silently enlarge the plan.

## Composite, port-group, and instance-pool identity

Logical composites retain definition hash, exact expanded member paths, and
direction-preserving export mappings. All member paths resolve to primitive
nodes. Logical mappings are provenance and boundary identity; they do not hide
primitive nodes, effects, queues, or allocations.

A port-group entry pins its template hash and every derived member ID,
ordinal, and port-contract hash. Ordinals form a unique finite range
independent of registry or scheduler iteration order.

An instance-pool entry pins:

- template and derived-identity hashes;
- maximum live and queued instances;
- admission and supervision descriptor pins;
- per-instance resource budget and maximum runtime ticks;
- exact grant-ID set and implementation-set hash;
- finite correlation slots;
- worst-case aggregate resource reservation; and
- maximum child-node and child-cord counts.

Correlation slots cover all live plus queued instances. Worst-case reservation
covers at least `maximum_live * per_instance_budget` using checked arithmetic.
Child, cord, timer, transport, checkpoint, and evidence maxima are therefore
fixed before execution and never derived from scheduler order.

## Canonical identity

The identity is the specification 003 canonical semantic hash of
`conduit/execution-plan` schema 1. The identity field itself is excluded.

The top-level descriptor contains source, resolver, creation-time, and
aggregate-budget fields plus a canonical set of domain-separated leaf fact
hashes. Leaf descriptors cover each host observation, concrete resource
binding, artifact, node, required resource/effect relation, cord, authority
binding, composite/member/export relation, port-group/member relation,
instance pool/grant relation, and unresolved selector.

Canonical set ordering makes identity independent of registry discovery,
hash-map, filesystem, and input collection order. Wall clock and run-start
time are not identity inputs. The exact named creation-time observation is.
Duplicate canonical facts are invalid.

`conduit-core` streams this form without an allocator. The caller supplies one
`SemanticHash` scratch slot per leaf fact. `conduit-runtime` provides an
allocator-aware convenience wrapper that sizes this scratch exactly. Scratch
size changes memory strategy, not plan identity.

## Portable validation and staleness

`validate_execution_plan` returns the first failure in deterministic
collection order. Before any node starts it checks:

1. supported schema and empty unresolved set;
2. descriptor/path syntax and duplicate IDs;
3. host-report and artifact references;
4. node implementation, contract, and resource pins;
5. endpoint existence, direction, type identity, and finite queue accounting;
6. complete required authority and run-start validity;
7. composite, export, port-group, and instance-pool references and maxima;
8. checked aggregate budget; and
9. canonical plan identity using caller-owned scratch.

The validation context contains only supported schema and a named deterministic
run-start time observation. It does not alter identity. A host report or grant
that was valid during planning but is stale or expired at run start causes
rejection; the executor never silently refreshes or substitutes it. A resolver
must create a new plan for new observations or selections.

Hosted validation may collect several diagnostics and consult descriptor or
artifact stores. The core-compatible profile needs only the self-contained
pins and scratch described above. Neither profile executes code while
validating.

## Diagnostics and fixtures

| Code | Meaning |
|---|---|
| `CND-PLN-001` | unsupported plan schema |
| `CND-PLN-002` | canonical identity mismatch |
| `CND-PLN-003` | malformed exact descriptor |
| `CND-ID-002` | duplicate identity |
| `CND-PLN-004` | dangling exact reference |
| `CND-PLN-005` | unresolved selector in a runnable plan |
| `CND-ART-001` | selected artifact is absent |
| `CND-HST-002` | selected host observation is stale |
| `CND-PLN-006` | allocation overflow or aggregate budget exceeded |
| `CND-AUT-007` | required effect/grant/binding is absent or invalid |
| `CND-FLW-001` | queue allocation is zero, inconsistent, or unbounded |
| `CND-PRT-001` | endpoint direction is invalid |
| `CND-TYP-001` | pinned endpoint type contracts differ |
| `CND-PLN-007` | caller identity scratch is too small |

`conformance/c2/execution-plan-v1.tsv` freezes valid minimal and nested plans,
duplicate IDs, dangling endpoints, hash mismatch, invalid capacity, unresolved
implementation, missing artifact, over-budget allocation, absent and expired
grants, stale host report, unsupported version, order-independent canonical
identity, and bounded instance-pool behavior.

## Normative requirements

| ID | Obligation |
|---|---|
| PLN-001 | Keep source, semantic contracts, exact plans, observations, evidence, and presentation as distinct identities |
| PLN-002 | Pin every implementation, artifact, host report, contract, resource, queue, and grant needed to run |
| PLN-003 | Admit no unresolved selector to a runnable plan |
| PLN-004 | Compute identity canonically without discovery, map, filesystem, scheduler, or wall-clock order |
| PLN-005 | Validate structure, references, directions, hashes, budgets, authority, and version before start |
| PLN-006 | Reject host-report or grant staleness at creation and run start |
| PLN-007 | Account every live queue and dynamic-pool maximum finitely |
| PLN-008 | Preserve logical composite/export and exact expanded provenance |
| PLN-009 | Encode port-group and instance-pool identities and maxima before runtime |
| PLN-010 | Never describe an ExecutionPlan as code, universal bytecode, a native executable, or universal host compatibility |
