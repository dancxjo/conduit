# Port and typed configuration contracts current form

Status: normative current contract

Port descriptor schema marker: `0`

Configuration-field descriptor schema marker: `0`

## Purpose

A live port is a typed temporal boundary. It is not just a type name.
Configuration is a separate typed schema and is never patchable as a cord.
This specification freezes their current form fields, encodings, directional
checks, diagnostics, defaults, mutation timing, identity, and redaction rules.

It builds on exact type references from
[`005-type-contracts.md`](005-type-contracts.md) and the directional
algebra from
[`004-directional-compatibility.md`](004-directional-compatibility.md).

## PortContract

Every `PortContract` contains these semantic fields:

| Field | Meaning |
|---|---|
| `id` | stable local port identifier |
| `direction` | `input` or `output` |
| `value_type` | exact `TypeContractRef` with ID, schema revision, and hash |
| `presence` | whether a complete assemblage must connect the port |
| `connections` | permitted number of attached cords |
| `values` | permitted logical values per activation |
| `delivery` | stream, latest state, finite batch, artifact reference, or control |
| `temporal` | atemporal, progressive, or committed value stability |
| `terminal` | finite, open-ended, or either natural terminal behavior |
| `sensitivity` | public, restricted, or secret ceiling |
| `flow.loss` | lossless-only or TypeContract-proven loss acceptance |

Connection cardinality and value cardinality are deliberately distinct. A
single cord may carry many values; many cords do not turn a single-valued
contract into a stream.

The descriptor kind is `conduit/port-contract`, current schema. All fields
above participate in canonical semantic identity. `value_type` is a map of
`contract_id`, `schema_version`, and the 32 exact semantic-hash bytes. Enum
spellings are the lowercase hyphenated spellings in this document.
`conformance/c2/port-contract.tsv` freezes the complete input descriptor
hash and round-trips each external spelling.

The older C1 `node-contract` canonical vector remains immutable evidence for
canonical-form current form. It is not silently reinterpreted as this complete C2
schema. A schema-aware migration must add every new field explicitly and
produce a new node-contract identity.

## Directional connection checks

A connection query always names a consumer input and producer output. Checks
run in deterministic order:

1. direction is output to input;
2. the nested TypeContract decision is compatible;
3. every producer value count is accepted by the consumer;
4. delivery shape is equal;
5. temporal behavior is accepted;
6. terminal behavior is accepted;
7. consumer sensitivity is at least producer sensitivity; and
8. consumer loss acceptance includes producer loss behavior.

A progressive consumer accepts committed values, but a committed consumer
rejects a producer that may emit provisional values. `terminal=either` accepts
finite or open-ended producers; the other values are exact. Public consumers
cannot accept restricted or secret producers without an explicit
declassification adapter and authority.

Specification 010 freezes the authority binding and recording/presentation
rules. No grant permits an implicit sensitivity downgrade.

Substitution checks use the variance direction from `COM-008`. A candidate may
not make an optional port required or narrow the permitted connection counts.
Input type decisions ask whether the candidate accepts the required input;
output type decisions ask whether the required output boundary accepts the
candidate. Other fields use the same conservative directional rules.

Stable port reasons are:

| Reason | Meaning |
|---|---|
| `port-accepted` | every directional check passed |
| `port-direction-mismatch` | endpoints are not output to input |
| `port-type-mismatch` | nested TypeContract decision is not compatible |
| `port-presence-mismatch` | candidate made an optional port required |
| `port-connection-cardinality-mismatch` | candidate narrowed cord counts |
| `port-value-cardinality-mismatch` | producer may emit an unaccepted count |
| `port-delivery-mismatch` | delivery shapes differ |
| `port-temporal-mismatch` | temporal behavior is not accepted |
| `port-terminal-mismatch` | natural terminal behavior is not accepted |
| `port-sensitivity-violation` | data would cross into a weaker boundary |
| `port-flow-constraint-mismatch` | producer loss behavior is not accepted |

The decision retains both complete port operands and the complete nested
TypeContract decision. The portable plan validator maps these reasons to
stable `CND-PRT-*`, `CND-TYP-*`, `CND-AUT-*`, and `CND-FLW-*` diagnostics.

Specification
[`027-implicit-satisfaction.md`](027-implicit-satisfaction.md)
operationalizes accepted non-exact port relations as complete immutable
proofs. It adds no new implicit port rule: every current-form field plus
authority, concrete representation/ownership, and bounded flow remain
separate required obligations.

## Flow and lifecycle ownership

Every resolved cord still requires a positive finite item capacity. current form
port flow constraints say only whether loss must be absent or may be admitted
after a TypeContract proves the required trait. Exact pressure transitions,
capacity units, watermarks, fairness, sampling, and coalescing are defined by
[`007-bounded-flow-policy.md`](007-bounded-flow-policy.md).

current form terminal fields distinguish finite natural completion from an
open-ended producer. Cancellation scopes, drain/abort, race precedence, and
complete lifecycle state machines belong to #8. Those issues must version this
descriptor if they change an existing field's meaning.

## Typed configuration

`ConfigContract` is a list of `ConfigFieldContract` values on a node contract.
It is not stored among input or output ports. A field contains:

| Field | Meaning |
|---|---|
| `key` | stable local configuration key |
| `value_type` | exact domain-owned TypeContract reference |
| `requirement` | required, optional, or canonically defaulted |
| `sensitivity` | public, restricted, or secret handling |
| `mutability` | `pre-start` or `runtime` |
| `identity` | semantic descriptor identity or exact plan identity |

The field descriptor kind is `conduit/config-field-contract`, current schema.
A public default is encoded as the exact canonical value. Required and optional
fields omit the non-applicable default field. Changing a default changes the
field-contract semantic hash. A `conduit/config-contract` descriptor stores the
canonical set of exact field-contract hashes, so source field order does not
change schema identity.

Pre-start resolution rejects duplicate or unknown assignments, missing
required fields, incompatible or indeterminate types, and sensitivity
violations. Defaults are applied in contract order. An omitted default and an
explicit value canonically equal to that default produce the same semantic
configuration hash.

Runtime updates are separate evidenced operations. They reject fields declared
`pre-start`; no ordinary assignment path silently mutates them.

## Secrets and identity

current form forbids inline defaults for restricted or secret fields. It also
forbids protected values from participating directly in semantic descriptor
identity. Their field contract remains semantic, while the exact secret binding
belongs to plan identity and later authority resolution.

Hosted protected values format only as `[REDACTED]`. Configuration errors carry
stable code and key, never supplied value material. Exposing secret bytes is an
explicit implementation-boundary operation. Debug output, display output,
semantic hashes, conformance output, and diagnostics do not reveal actual
secret material.

Fields with `identity=semantic` must be public. `identity=plan` does not mean
non-semantic execution: it means the semantic contract defines the slot while
the exact resolved value or secret binding is pinned by the execution plan.

## Explicit adapters

An adapter is a node with an input PortContract accepted by the original
producer and an output PortContract accepted by the final consumer. The
unadapted endpoints remain incompatible. The conformance fixtures include both
valid adapter boundaries and the direct type mismatch; no adapter is inserted
implicitly.

## Fixtures

- `conformance/c2/port-contract.tsv` covers direction, type, presence,
  connection cardinality, value cardinality, delivery, temporal, terminal,
  sensitivity, flow, valid committed-to-progressive acceptance, and both
  explicit adapter boundaries.
- `conformance/c2/config.tsv` covers canonical default application,
  explicit/default equivalence, protected-value redaction, pre-start mutation,
  and sensitivity rejection.

The Rust reference consumes these files directly and asserts ordered operands,
outcome, stable reason or code, enum round trips, and canonical hashes.

## Normative requirements

| ID | Obligation |
|---|---|
| PRT-001 | Encode every current form port field in canonical semantic identity |
| PRT-002 | Keep connection and value cardinality distinct |
| PRT-003 | Ask output-to-input compatibility directionally |
| PRT-004 | Retain the nested reasoned TypeContract decision |
| PRT-005 | Reject delivery, temporal, terminal, and sensitivity mismatches |
| PRT-006 | Preserve input contravariance and output covariance in substitution |
| PRT-007 | Keep adapters explicit and never insert them during compatibility |
| PRT-008 | Keep every live cord finite and expose conservative loss constraints |
| CFG-001 | Keep typed configuration separate from live ports |
| CFG-002 | Give every field exact type, requirement, sensitivity, and mutability |
| CFG-003 | Apply public defaults canonically and preserve explicit/default identity |
| CFG-004 | Reject runtime mutation of pre-start fields |
| CFG-005 | Redact protected values from formatting, hashes, fixtures, and errors |
| CFG-006 | Put exact protected bindings in plan identity, not semantic hashes |
| CFG-007 | Reject duplicate, unknown, missing, mistyped, and over-sensitive values |
