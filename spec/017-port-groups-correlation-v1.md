# Compile-time port groups and correlation identity version 1

Status: C2/C3 normative semantic contract

Port-group contract version: 1

Correlation contract version: 1

ExecutionPlan port-group representation: schema 2

This specification reconciles the grammar and lowering forms frozen by
specifications 014 and 015 with complete port contracts, exact plans, and
immutable evidence. It also defines the identity families that must survive
local and remote execution, retries, resume, replication, and plan-epoch
transitions.

A compile-time group is a finite collection of ordinary ports. It is not a
runtime array-valued port, handler table, dynamic route registry, replicated
population, or permission to allocate work. Correlation metadata describes
relationships between work; it is not a scheduler, clock, or authority grant.

## Frozen source mapping

The following mapping is exhaustive for specification 014's `port-group`
production:

| Frozen source fact | Semantic obligation | Lowered/plan fact |
|---|---|---|
| group `word` | stable identity in its owning namespace | logical group path and template hash |
| `input` or `output` | direction of every member's complete contract | validated direction and plan-v2 direction |
| qualified contract name after `:` | exact complete `PortContract`, not a value type | contract ID and exact semantic hash on every member |
| `keyed` | authored fixed membership | exact authored keys and key-token spans |
| `member word` | stable member key in source order | key, ordinal, derived ID, and exact member origin |
| `indexed` | derived fixed membership | decimal keys and ordinals for exactly `0..maximum` |
| `max number` | positive finite semantic maximum | lowered maximum and explicit plan-v2 maximum |
| containing definition | logical ownership and export boundary | logical group path distinct from expanded member path |

Source order is the only ordering input for keyed members. Indexed ordering is
ascending numeric index. Catalog insertion, hash-map iteration, registry
discovery, scheduler order, transport discovery, and host observations MUST
NOT reorder members or allocate identities.

The source spelling `routes[home]` is an authored endpoint spelling. Persisted
semantic records retain group path and member key as separate fields; they do
not require brackets to become valid portable `Id` or `InstancePath` bytes.

## Reconciliation findings

| Frozen-boundary finding | Classification | Resolution |
|---|---|---|
| keyed members reused the containing group span | implementation bug preserving grammar-v1 meaning | AST annotation and lowering now retain the exact key-token span |
| authored direction was not checked against the referenced contract | missing validation/diagnostic | lowering now checks the complete contract direction and emits `CND-LWR-010` |
| lowered members lacked explicit ordinal and separate logical/expanded paths | missing persisted lowering facts | compatible hosted record extension; the member hash now includes ordinal |
| plan schema 1 had no explicit maximum or direction | insufficient persisted plan schema | schema 1 stays frozen; plan schema 2 adds both fields and a new group hash domain |
| lowering-v1 omits general cords, child relationships, exports, and bindings | genuinely insufficient frozen lowering schema | linked versioned correction remains owned by issue #64; no lowering-v1 reinterpretation |
| evidence fields existed without a complete allocator/scope/propagation contract | missing normative contract and fixtures | the correlation family table and conformance cases below supply it |
| fixtures did not prove order independence, maxima, nested exports, retry/resume, or transition identity | missing conformance coverage | manifest revision 5 adds positive, negative, boundary, migration, and forbidden-allocator cases |

## Group and member identity

Every group has:

- a logical path in its owning source boundary;
- shape (`keyed` or `indexed`);
- positive `u16` maximum;
- direction;
- exact complete port-contract ID and semantic hash;
- ordered finite membership; and
- content-identified group source origin.

Every expanded member has:

- the logical group path;
- key or decimal index;
- zero-based ordinal;
- a stable derived local port spelling;
- the same validated direction and exact complete port-contract pin;
- an expanded semantic path;
- the group maximum; and
- provenance distinguishing an authored key token from an indexed derivation.

For keyed groups, member origin is the exact key token span. For indexed
groups, no key token is invented: the member points to the exact group origin
and records that its key is derived. Source origins and spans are annotations
and do not enter semantic hashes.

The lowered member identity uses the versioned
`conduit/lowered-group-port` domain over logical group path, derived port
spelling, ordinal, direction, exact port-contract ID/hash, and maximum.
Changing any of those facts changes the member and aggregate lowered identity.
Trivia, source URI spelling after canonical module resolution, source-map
collection order, and presentation do not.

The semantic maximum bounds possible authored membership. A keyed group may
contain fewer members than its maximum. An indexed group contains exactly its
maximum. Neither form admits members after run start.

## Complete contract validation and exports

The semantic catalog returns an exact complete port-contract reference,
including its direction and canonical semantic hash. Lowering rejects a
separately authored direction that disagrees with that contract as
`CND-LWR-010`. The hash pins presence, connection/value cardinality, delivery,
temporal and terminal behavior, sensitivity, flow loss constraints, and exact
type identity from specification 006.

Each expanded member participates in ordinary compatibility, flow,
sensitivity, terminal, authority, and connection-cardinality checks. There is
no group-level bypass and no implicit adapter.

A composite exposes a group member only through an explicit ordinary export.
Nested export chains retain every logical boundary and the final expanded
member. Direct access to an unexported child remains `CND-SRC-009`.
Specification 009's complete-contract export rules apply unchanged.

The frozen lowering-v1 record can now retain exact group/member facts and
source maps, but it does not retain general cord/export/binding topology.
That broader, explicitly versioned correction is owned by issue #64. This
specification does not reinterpret lowering-v1 or create a second export
model.

## ExecutionPlan schema 2 correction

ExecutionPlan schema 1 remains readable and retains its frozen canonical
identity. Its port-group entry contains only logical instance, template hash,
and member IDs/ordinals/contract hashes; the newly exposed in-memory
`maximum` and `direction` fields are ignored while hashing and validating a
schema-1 plan.

ExecutionPlan schema 2 retains the same structure and adds normative,
explicit group `maximum` and `direction`. The group leaf uses the
`conduit/plan-port-group-v2` domain. Portable validation requires:

- a positive maximum;
- one or more members and no more members than the maximum;
- valid, unique member IDs;
- unique ordinals forming exactly `0..member_count`; and
- one group identity per logical instance.

Migration from schema 1 to schema 2 is not inference from member count.
The planner MUST supply maximum and direction from the exact lowered semantic
group contract, then compute a new schema-2 plan identity. A schema-1 keyed
group with two members might have a maximum larger than two; treating count as
maximum would silently narrow source semantics.

## Plan and evidence paths

Plans retain group identity separately from member identity. Evidence uses:

- `logical_template` for the owning logical composite or group template;
- `subject` for the exact expanded member, concrete instance, or generation;
- exact plan identity for the immutable plan epoch;
- `EventCorrelation` for request/session/work/attempt families; and
- `EventRelations.caused_by` and `derived_from` for event causation.

For example, a route group may use logical path `root/routes`, member path
`root/routes/home`, and later replicated execution path
`root/workers/generation.g3/instance.i7/attempt.a2`. These are portable path
components, not array lookup or scheduler indexes.

## Correlation identity families

Each identity family has one allocator and one uniqueness boundary. Allocators
may be implemented by different languages or transports, but allocation is an
explicit protocol act and never an ambient ordering side effect.

| Family | Allocator | Scope and lifetime | Uniqueness boundary | Sensitivity and propagation | Serialization |
|---|---|---|---|---|---|
| request | request origin | one initiated request through terminal reply/failure | origin namespace | metadata; copied end-to-end where policy permits | portable `Id` |
| exchange | request/reply protocol owner | one request/reply exchange | request plus protocol namespace | metadata; copied on request and reply | portable `Id` |
| session | session owner | one logical session until explicit terminal/replacement | session-owner namespace | policy-classified metadata; copied across its work | portable `Id` |
| session epoch | session owner | one monotonic logical epoch within a session | session ID plus numeric epoch | metadata; propagated with session | unsigned 32-bit integer |
| work unit/job | work submitter | one logical job across resume and retries | submitter namespace | metadata; copied to attempts/checkpoints | portable `Id` |
| attempt | supervisor | one execution attempt until terminal | work unit plus supervisor namespace | metadata; replaced on retry/restart | portable `Id` |
| event causation | run recorder/observer | one direct event relation retained forever | exact run evidence stream | evidence metadata; references exact event ID | portable `Id` in `caused_by` |
| broader correlation | initiating boundary | explicitly declared related activity | allocator namespace | policy-classified metadata; copied only by declared propagation | portable `Id` |
| idempotency | caller owning duplicate suppression | caller-declared replay window | target operation plus caller namespace | potentially sensitive metadata; only to enforcing boundaries | portable `Id` |
| checkpoint | checkpoint writer | one durable progress record | work unit plus checkpoint store | metadata; copied on resume | portable `Id` |
| transport | transport boundary | one transport operation or hop | transport endpoint namespace | hop metadata; may change at each explicit boundary | portable `Id` |
| logical template | planner | lifetime of one semantic template identity | source/lowered semantic closure | semantic metadata; retained across instances | portable `InstancePath` |
| concrete instance | finite pool controller | one admitted concrete instance | template plus generation | metadata; stable through that instance | portable `InstancePath` |
| generation | replication/transition controller | one finite population generation | logical template | metadata; changes on replacement generation | portable path component |
| plan epoch | resolver/transition controller | one immutable exact ExecutionPlan | run lineage | exact semantic hash; every event pins it | `SemanticHash` |

The identity categories are semantically distinct even when represented by the
same portable identifier grammar. A producer MUST NOT copy one category's
value into another field as a substitute. A retry keeps request, exchange,
session, work, broader correlation, and idempotency when applicable, but
allocates a new attempt. Resume keeps the work unit and allocates a new
attempt; it references, but does not become, the checkpoint. A transport hop
retains applicable end-to-end context and allocates its own transport ID.

Plan transition creates a new exact plan identity. It does not rewrite prior
events. A stable logical template may survive, while generation, concrete
instance, attempt, and plan epoch identify the new work. Issue #57 owns the
bounded transition protocol; issue #60 owns finite runtime populations. Both
consume these identities without redefining them.

## Ordering, allocation, and negative rules

Append sequence, observer sequence, scheduler wake order, registry iteration,
map order, transport discovery, wall time, and domain time are observations or
orders, not identity allocators. Timestamps never establish causation.

The following are invalid:

- deriving request, work, attempt, generation, or event IDs solely from a
  timestamp;
- using queue position or scheduler activation count as an identity;
- assigning member ordinals from registry or map iteration;
- assigning remote identity from transport discovery order;
- reusing a work-unit ID as its retry attempt;
- reusing an attempt after restart or checkpoint resume;
- treating idempotency as authorization or broader correlation as causation;
- treating plan creation time as plan-epoch identity; or
- mutating an active plan while retaining its prior plan identity.

## Diagnostics

| Code | Meaning |
|---|---|
| `CND-LWR-010` | authored group direction conflicts with the exact complete port contract |
| `CND-PLN-003` | plan-v2 group maximum, membership, ID, ordinal, or direction is malformed |
| `CND-EVD-002` | evidence correlation or path identity is malformed or collapsed |
| `CND-SRC-008` | source maximum is zero, overflowed, absent, or membership exceeds it |
| `CND-SRC-009` | group member crosses a composite boundary without an explicit export |

## Conformance

`conformance/c2/port-group-correlation-v1.json` is language-neutral and
normative. It covers keyed/indexed expansion, exact member spans, complete
direction validation, deterministic order, maxima, nested export source forms,
semantic hash sensitivity and trivia stability, plan-v1 preservation and
plan-v2 migration, local/remote request/reply propagation, retry and resume,
generation/plan-epoch separation, and forbidden ambient allocators.

## Normative requirements

| ID | Obligation |
|---|---|
| PGC-001 | Treat a group as a finite pre-run collection of ordinary complete ports |
| PGC-002 | Retain stable group path, member key/index, derived identity, and deterministic ordinal |
| PGC-003 | Retain exact authored member spans and distinguish indexed derivation provenance |
| PGC-004 | Validate direction against the exact complete PortContract and pin its full semantic hash |
| PGC-005 | Preserve positive semantic maximum through lowering and plan schema 2 |
| PGC-006 | Apply ordinary compatibility, sensitivity, flow, terminal, authority, and export rules to every member |
| PGC-007 | Keep logical group, expanded member, concrete instance/generation, plan, and evidence identities distinct |
| PGC-008 | Make order and identity independent of map, registry, scheduler, transport, and discovery order |
| PGC-009 | Preserve plan schema 1 and migrate to schema 2 only with exact maximum and direction inputs |
| PGC-010 | Preserve explicit group-member exports through every composite boundary without bypass |
| COR-001 | Keep request, exchange, session/epoch, work, attempt, causation, broader correlation, and idempotency distinct |
| COR-002 | Define explicit allocator, scope, lifetime, uniqueness, serialization, sensitivity, and propagation per identity family |
| COR-003 | Retain applicable end-to-end context across local and remote cords while allocating hop-local transport identity |
| COR-004 | Allocate a new attempt for retry, restart, and checkpoint resume without replacing work identity |
| COR-005 | Retain logical template while distinguishing concrete instance, generation, and immutable plan epoch |
| COR-006 | Never allocate identity or infer causality from clock, scheduler, registry, map, network, or discovery order |
| COR-007 | Preserve exact plan and correlation context in immutable evidence across transitions |
