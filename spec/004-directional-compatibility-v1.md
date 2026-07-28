# Directional compatibility and migration version 1

Status: stable

Compatibility algebra version: 1

## Purpose

This specification defines what Conduit means by exact identity, acceptance,
backward compatibility, forward compatibility, substitution, and migration.
It freezes the result algebra implemented by `conduit-core` and the C1
compatibility fixtures.

Compatibility is a question with named roles. It is not similarity, matching
version numbers, a bare boolean, or a promise that an adapter can probably be
found later.

## Exact subjects

Every subject in a compatibility query identifies:

- descriptor kind;
- exact kind-scoped schema revision; and
- exact canonical semantic hash.

Display names, source locations, providers, artifact locations, and
presentation metadata do not substitute for this identity.

## Queries

Version 1 defines these directional questions:

### Exact

```text
exact(left, right)
```

Exact requires equal descriptor kind, schema revision, and semantic hash. It is
reflexive, symmetric, and transitive.

### Reader accepts writer

```text
reader_accepts_writer(reader, writer)
```

This asks whether the reader accepts every valid document the writer may emit,
without silently discarding semantic information or changing missing-value
meaning.

### Consumer accepts producer

```text
consumer_accepts_producer(consumer, producer)
```

This asks whether every value, terminal condition, timing behavior, and
delivery behavior permitted by the producer is accepted by the consumer.
Domain-owned type meaning is decided by the owning domain provider.

### Candidate substitutes required

```text
candidate_substitutes_required(required, candidate)
```

This asks whether the candidate can replace the required contract at that
boundary for every valid surrounding use. It is stronger than one observed
successful connection.

### Migration

```text
migrates(source, target, migration)
```

This asks whether one exact, separately identified migration transforms every
valid exact source value into the exact target contract deterministically.

The query roles MUST appear in APIs, diagnostics, plans, and conformance
claims. APIs MUST NOT accept an unlabeled pair and call the answer
“compatible.”

## Outcomes and classes

Every decision has one outcome:

| Outcome | Meaning |
|---|---|
| compatible | the requested directional relation is proven |
| incompatible | the requested directional relation is disproven |
| indeterminate | a named provider or additional fact is required |

Indeterminate is not a cautious spelling of incompatible. A resolver MAY ask
the named provider and issue a new decision. It MUST NOT silently coerce
indeterminate to true or false.

A compatible decision also has one class:

| Class | Meaning |
|---|---|
| exact | exact kind, revision, and semantic hash |
| accepted | same-version non-identical writer is accepted by this reader |
| backward-compatible | newer reader accepts older writer |
| forward-compatible | older reader accepts newer writer |
| substitutable | candidate replaces required contract at the stated boundary |
| migratable | exact deterministic total migration is available |

“Backward” and “forward” describe reader/writer direction:

- backward-compatible means a **new reader reads old output**;
- forward-compatible means an **old reader reads new output**.

A version number alone proves neither. The same pair can be compatible in one
direction and incompatible in the other.

Every decision carries a stable reason and, where possible, the exact field,
port, grant, effect, or other local subject. Conduit does not expose a
reason-free `is_compatible` API.

## Conservative record-schema acceptance

The portable reference rule applies to descriptor and evidence records.
Fields may appear in any source order.

A reader accepts a writer only when all of the following hold:

1. descriptor kinds match;
2. every reader-required field is always emitted by the writer or has the same
   declared missing-value semantics;
3. every writer value contract is accepted directionally by the corresponding
   reader field;
4. when a writer may omit a known field, reader and writer defaults are
   identical;
5. every additional writer field is either understood or preserved under an
   explicit unknown-field policy; and
6. neither schema is malformed or ambiguous.

Unknown semantic fields are rejected by default. A preserving reader may carry
an unknown canonical field through losslessly, but it does not thereby claim
to interpret or execute that field.

Changing a default is semantic even if the wire field is absent. It is
incompatible whenever a writer may omit that field, unless an explicit
migration accounts for the change.

Value contracts are directional. The reader may:

- require exact value-contract identity;
- name exact producer contracts already approved by a domain provider; or
- return indeterminate and ask the owning provider.

A reader that accepts a narrow producer may not necessarily accept a wider
producer. Silently treating structural resemblance or a matching primitive
shape as semantic acceptance is forbidden.

## Type contracts

Type meaning belongs to domain profiles. A type provider evaluates:

- units and scale;
- coordinate frame and clock;
- shape and size bounds;
- value constraints;
- sensitivity and retention;
- replacement, disposal, and ordering traits;
- terminal and error meaning; and
- explicit adapters.

Without an exact match or a provider decision, type compatibility is
indeterminate. Conduit core does not infer equivalence from names, JSON shape,
Rust layout, Python class, WASM type, or byte length.

An adapter is an explicit node and contract. Its existence does not make the
unadapted endpoints directly compatible.

## Port contracts

For an output patched to an input:

- the input type consumer must accept the output type producer;
- direction must be output to input;
- delivery, ordering, temporal, terminal, and loss behavior must be accepted;
- bounded-flow requirements must have a satisfiable exact resolution; and
- sensitivity and authority must not be weakened.

For substitution:

- candidate inputs are contravariant: they accept at least what required inputs
  accept;
- candidate outputs are covariant: they produce no more than required outputs
  permit;
- required ports cannot disappear;
- new required inputs cannot be introduced;
- optionality and cardinality cannot make a formerly valid patch invalid; and
- delivery or terminal meaning cannot change without an explicit adapter.

## Node contracts

A candidate node substitutes a required node only when:

- its port boundary substitutes under the port rules;
- configuration accepted by the required boundary remains accepted;
- it requests no additional effects or authority;
- it does not weaken cancellation, terminal, checkpoint, or determinism
  guarantees;
- its typed failures are accepted by the surrounding scope; and
- its resource or portability requirements do not contradict the query
  profile.

Implementation readiness is not node compatibility. A compatible node may have
no usable implementation on the observed host; an available implementation may
still fail to satisfy the semantic contract.

## Panel documents and exported composites

A panel definition is compatible at an exported node boundary when that
exported composite substitutes for the required node contract, including
effects, authority, lifecycle, and flow.

Internal nodes, cords, implementation selections, and layout may change without
breaking an exported boundary. Such changes still produce a new semantic hash
when they affect validation, resolution, authority, resources, or execution.
Presentation-only changes do not.

Compatibility of one export does not imply compatibility of every export or of
the panel document as a whole. Claims name the exact root or export path.

## Execution plans

Execution evidence and resumable state pin an exact execution plan. A plan is
not considered interchangeable merely because it has similar topology.

Changes to implementations, artifacts, host reports, bindings, grants,
resource budgets, flow policy, lifecycle policy, or any exact semantic
dependency create a different plan identity.

Whether a runtime can execute a plan is a separate directional
runtime-accepts-plan claim naming the runtime profile and plan encoding
versions. Plan migration, if ever admitted, is explicit and produces a new
plan; it does not rewrite prior evidence.

## Execution-event schemas

Event compatibility uses reader-accepts-writer direction.

- Sequence, run, plan, subject, causation, time, terminal, loss, and derivation
  meanings cannot be weakened or reinterpreted.
- A new reader may accept old events when new fields have safe explicit
  defaults or remain optional.
- An old reader accepts new events only when it can preserve unknown semantic
  fields and the consuming operation does not claim to interpret them.
- Unknown event variants are not silently mapped to a familiar variant.
- Immutable historical events retain their original schema and plan identity.

Mutable projections may migrate independently; they do not alter original
evidence.

## Migration identity

A migration names:

- stable migration ID;
- exact migration semantic hash;
- exact source descriptor kind, revision, and semantic hash;
- exact target descriptor kind, revision, and semantic hash;
- determinism contract; and
- totality or typed-failure contract.

Automatic compatibility requires an exact source and target match, deterministic
behavior, and total coverage. Partial, heuristic, interactive, or
context-sensitive transformations may be useful tools, but they do not prove
general migratability.

Migration is separately attributable execution. Its output has the target
identity; evidence records the migration identity and original source. A
migration never changes the identity or meaning of stored source data.

Migration composition is explicit. The availability of A→B and B→C does not
permit a resolver to claim A→C without identifying and validating that exact
composition.

## Unsafe changes

These are incompatible unless a stronger domain-specific proof or explicit
migration says otherwise:

- removing or renaming a required field or port;
- adding a required reader field without safe missing-value semantics;
- adding writer fields to a rejecting old reader;
- changing defaults while omission is possible;
- narrowing accepted input or widening produced output;
- changing unit, clock, coordinate frame, delivery, ordering, loss, or terminal
  meaning;
- adding effects or requested authority;
- weakening cancellation, determinism, checkpoint, or safety guarantees;
- reusing a tag, field, variant, or port for new meaning; and
- treating an unknown critical field as an annotation.

## Fixtures

The normative fixtures are:

- `conformance/c1/compatibility-v1.tsv` for exact, backward, forward,
  incompatible, and indeterminate record-schema decisions; and
- `conformance/c1/migration-v1.tsv` for accepted, mismatched,
  nondeterministic, and partial migrations.

The Rust reference consumes these files directly. They are line-oriented UTF-8
with tab-separated columns so other implementations can consume them without a
language-specific harness.

## Evolution

Version 1 reason spellings and class meanings are stable. New reason variants
may be added without changing an existing reason. New compatibility relations
require a new algebra version if they alter the meaning of an existing query,
outcome, or class.

Compatibility across canonical-form or compatibility-algebra versions is
itself an exact directional claim. A new implementation does not reinterpret
old decisions.

## Normative requirements

| ID | Obligation |
|---|---|
| COM-001 | Name the exact subjects and directional roles in every query |
| COM-002 | Return compatible, incompatible, or indeterminate with a reason |
| COM-003 | Reserve exact for equal kind, revision, and semantic hash |
| COM-004 | Define backward as new-reader/old-writer acceptance |
| COM-005 | Define forward as old-reader/new-writer acceptance |
| COM-006 | Reject silent default changes and unknown semantic-field loss |
| COM-007 | Delegate domain type meaning to the owning provider |
| COM-008 | Apply contravariant inputs and covariant outputs for substitution |
| COM-009 | Keep plan and evidence identities exact |
| COM-010 | Identify migrations separately with exact source and target |
| COM-011 | Require deterministic total migration for general migratability |
| COM-012 | Preserve indeterminate as distinct from incompatible |
