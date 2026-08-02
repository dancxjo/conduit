# Temporal modality contract current form

Status: normative semantic contract

Descriptor schema marker: `0`

## Purpose and boundary

The common temporal surfaces lower to one exact `TemporalModalityContract`:

| Source surface | Meaning |
|---|---|
| `T` | one ordinary value followed by its normal boundary |
| `T...|` | zero or more values with a normal closing boundary available |
| `T...` | zero or more values with no promised normal closing boundary |
| `$T` | an immediately observable current value plus latest replacement |

The punctuation is source spelling only. The semantic descriptor retains the
exact item `TypeContractRef`, cardinality, closing boundary, initial
availability, retention, replay, and replacement behavior. Payload optionality
belongs to `T`; it is not another temporal modality.

The descriptor does not select an implementation, provider, device, host,
artifact, allocation, queue, pressure policy, clock, authority, or grant. It
does not allocate or execute anything.

## Complete field profiles

| Surface | Cardinality | Closing | Initial | Retention | Replay | Replacement |
|---|---|---|---|---|---|---|
| `T` | `exactly-one` | `after-value` | `not-promised` | `none` | `none` | `no-replacement` |
| `T...|` | `zero-or-more` | `available` | `not-promised` | `none` | `none` | `no-replacement` |
| `T...` | `zero-or-more` | `absent` | `not-promised` | `none` | `none` | `no-replacement` |
| `$T` | `current-and-replacements` | `absent` | `immediate-current` | `latest-replacement` | `current-only` | `replace-latest` |

Only these four combinations are published. A mixed field combination fails
as `temporal-modality-invalid-combination`; an invalid item type fails as
`temporal-modality-invalid-item-type`. New punctuation or modality combinations
require a new reviewed semantic profile and conformance cases, not permissive
field mixing.

`T...|` exposes a normal closing boundary but does not promise that execution
will reach it. It implies no maximum item count, byte count, duration, queue,
or scheduler progress. Those facts remain separate explicit contracts.

`T...` has no normal closing promise. Failure, cancellation, provider loss,
and plan transition still use their ordinary explicit terminal and lifecycle
contracts.

## Current observation

`$T` promises that every admitted observer can read one current value
immediately. A later replacement becomes the current value and the displaced
value is not semantic history. An update with no observer is retained, and a
later observer receives the newest current value.

`current-only` replay means exactly that one current value. It does not imply
historical replay, durable persistence, equality deduplication, multi-writer
arbitration, a CRDT, a coalescing policy for a cord, or an evidence retention
policy.

Every temporal surface is observational. `$T` supplies no write or mutation
authority. Mutation requires a separate typed effect port and an independently
validated grant.

The hosted `CurrentValueCell<T>` retains exactly one resident value and a
monotonic replacement generation. Construction supplies the required initial
value, so observation is immediate. Observer presence is not stored: an update
with no observer remains current, and reconnect receives the newest value.
When multiple replacements occurred, the observer receives the newest value
plus an explicit skipped-replacement indication, never fabricated history.

Every replacement calls a separate `CurrentValueMutationAuthorizer` before
changing value or generation. A denied update and generation exhaustion leave
the cell unchanged. Equal values still advance the generation because `$T`
does not imply equality deduplication.

## Compatibility and conversions

Ordinary connection compatibility compares all temporal-modality fields in
addition to item-type compatibility. The exact portable comparison admits only
equal descriptors. Every cross-surface pair is incompatible and requires an
explicit named conversion node. Type-provider compatibility cannot change the
temporal modality.

Standard conversions must publish complete contracts and bounded
implementations. In particular:

- `flow/each` converts one bounded `List<T>` value to `T...|`;
- `flow/collect` accepts `T...|` only with explicit finite item and byte bounds;
- `state/sample` observes `$T` as one `T`;
- `state/changes` observes replacements from `$T` as `T...`;
- `state/hold` requires an explicit initial value and retains the latest input
  from `T...` as `$T`.

An open flow cannot reach `flow/collect` without an explicit finite
window/limit that produces a closing bounded input. A closing flow is not
resource-bounded merely because it has a normal closing boundary.

Pure modality-preserving lifting is an explicit node-contract fact and needs
law fixtures for value order and modality preservation. Effects, joins,
reducers, buffers, retries, clocks, and stateful nodes do not receive lifting
implicitly.

`ModalityLiftContract` binds one exact `NodeContract`, one named receiving
port, one named outgoing port, exact input and output `TypeContractRef` values,
an explicit set of admitted surfaces, an independent purity proof, and an
independent modality-law proof. Every field is semantic and identity-bearing.
An empty admitted set, malformed endpoint, wrong node descriptor, invalid type,
or missing proof fails closed.

Applying a declared lift changes only the item type. Cardinality, closing,
initial availability, retention, replay, and replacement are copied exactly.
An input type mismatch or unlisted surface is incompatible. Catalog state,
implementation code, callback shape, and a coincidentally matching pair of
ports cannot create a lift declaration.

## Canonical identity

`conduit/temporal-modality-contract` current schema hashes the exact item type
and every explicit modality field. All four surfaces have distinct identities.
Changing item meaning, cardinality, closing, initial availability, retention,
replay, or replacement changes semantic identity and invalidates dependent
plans.

## Normative requirements

| ID | Obligation |
|---|---|
| TMOD-001 | Lower the four source surfaces to the complete explicit field profiles in this specification |
| TMOD-002 | Keep payload optionality and item-type compatibility separate from temporal modality |
| TMOD-003 | Reject every implicit conversion among value, closing flow, open flow, and current observation |
| TMOD-004 | Make every modality field and the exact item type identity-bearing |
| TMOD-005 | Treat a closing boundary as distinct from item, byte, duration, queue, and progress bounds |
| TMOD-006 | Give `$T` immediate current observation, latest replacement retention, and current-only replay |
| TMOD-007 | Grant `$T` no mutation authority, history, durability, multi-writer arbitration, or CRDT semantics |
| TMOD-008 | Reject unpublished mixed field combinations with stable diagnostics |
| TMOD-009 | Require explicit bounded standard conversions and prohibit open-flow collection without an explicit finite window or limit |
| TMOD-010 | Permit modality-preserving lifting only through an identity-bearing node, endpoint, type, admitted-surface, purity-proof, and law-proof contract |
