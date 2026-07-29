# Conduit typed source lowering version 1

Status: C3 normative lowering contract

This document freezes the deterministic boundary between resolved `.panel`
source and hosted semantic descriptors. Lowering validates friendly source
literals against exact node-provided contracts, applies schema defaults, and
retains source provenance. It does not select an implementation, inspect a
host, resolve a secret, allocate a live cord, create a task, or produce an
ExecutionPlan.

Source bytes, CST tokens, source AST, resolved module graph, lowered semantic
descriptors, source maps, plans, run evidence, and presentation remain distinct
identities.

## Staged pipeline

Version 1 has these explicit stages:

1. UTF-8 bytes become lossless tokens and a CST with spans.
2. Strict parsing produces a typed source AST or a source diagnostic.
3. An explicit caller-supplied loader resolves names, imports, roots, content
   hashes, and pins into a closed `ModuleGraph`.
4. An explicit semantic catalog supplies exact node, port, configuration, and
   domain type contracts.
5. Literal validation converts source values into canonical semantic values.
6. Missing-value rules apply canonical schema defaults.
7. Lowering emits node descriptors, finite group ports, exact pool
   specifications, and a separate source map.

No stage evaluates arbitrary expressions or user code. The catalog is semantic
input, not implementation discovery or a live provider probe. A catalog may
use an already-loaded domain type provider to validate an opaque literal; an
unavailable required provider is a deterministic rejection.

## Exact source values

The grammar in specification 014 defines these values:

| Form | Source meaning |
|---|---|
| `true`, `false` | Boolean |
| `-128`, `0`, `127` | Signed base-10 source integer |
| `"text"` | Exact UTF-8 text |
| `bytes("00aBff")` | Exact bytes decoded from even-length hexadecimal |
| `name`, `ref("name")` | Unresolved ordinary reference |
| `contract("name")` | Unresolved contract reference |
| `secret("binding")` | Unresolved secret binding reference |
| `decimal("0.100")` | Exact base-10 text, without binary float or exponent |
| `list(true,false)` | Ordered source values |
| `record(a=1,b=2)`, `map(a=1,b=2)` | Unordered unique-key fields |

The source integer range is `i128`; the expected contract may narrow it.
Overflow of the expected contract is different from a wrong literal kind.
Record keys are unique and their semantic encoding is canonical key order.
Lists retain authored order. Exact decimals preserve all authored significant
digits for the domain contract to interpret; lowering never converts them to a
binary float.

A bare or constructed reference remains unresolved. Environment-variable
interpolation, filesystem reads, and implicit reference lookup are absent.

## Semantic catalog boundary

Each node schema contains:

- a stable node-contract ID;
- an exact type-contract ID, schema version, and semantic hash for every field;
- required, optional, or canonically defaulted missing-value behavior;
- sensitivity;
- mutability; and
- semantic-identity or plan-identity participation.

Hosted owned schemas copy these facts from allocator-free `conduit-core`
contracts. Domain catalogs additionally validate source literal shape and
meaning. They return canonical public values or one of the closed rejection
classes: wrong kind, overflow, invalid value, or provider unavailable.

Source-defined parameters become the configuration schema of their ordinary
definition contract. Their contract identity uses trivia-insensitive semantic
source identity. The resolved module content hash remains in source provenance
and pinning; it does not make comments or formatting semantic.

## Defaults and provenance

Every present lowered field carries exactly one provenance:

- `authored`: a public value was explicitly written;
- `schema-default`: omission applied a canonical contract default; or
- `plan-binding`: source retained a protected unresolved binding for later
  exact plan resolution.

Optional absent fields remain absent. Required absent fields fail. Defaults are
validated before use, including source-defined defaults. A source-defined
default maps to its authored module and span; a catalog default without source
has no invented source origin.

An explicit public value canonically equal to the declared default and omission
of that value produce the same semantic configuration descriptor. Provenance
and source location remain visible annotations and do not alter that identity.
`explain` reports every present field and its provenance without printing value
material.

Secret references are allowed only at a sensitivity-protected, plan-identity
field. They remain unresolved and do not enter semantic identity. Ordinary
`Debug`, diagnostics, and explain output render them as `[REDACTED]`. A nested
secret reference cannot be smuggled into a public record or list.

## Lowered descriptors and source maps

`LoweredSource` is a deterministic semantic compilation result, not a runnable
plan. It contains:

- node path, resolved contract ID, validated/defaulted configuration, and
  semantic hash;
- one ordinary exact port per finite keyed or indexed source group;
- one exact pool specification with its resolved template contract, finite
  maximum, admission bound, deadlines, idle limit, supervision/restart facts,
  and cleanup behavior; and
- a separate list of semantic-path-to-source-origin relationships.

An origin contains canonical module URI, exact module content hash, and
one-based source span. Source maps and provenance are not hashed as semantic
values. Node configuration fields are encoded in canonical key order. Records
are encoded in canonical key order and sets in canonical value order. Node,
port-group, and pool hashes use distinct versioned domains.

Keyed groups expand their authored members. Indexed groups expand exactly
`0..maximum`. Every member reuses the resolved complete PortContract identity.
Pool templates resolve through the same local/imported node-contract namespace
as ordinary instances. Missing group or template contracts fail closed.
Expansion creates neither handlers nor tasks and observes no host.

Specification
[`017-port-groups-correlation-v1.md`](017-port-groups-correlation-v1.md)
reconciles this frozen lowering form with exact keyed-member spans, complete
contract direction validation, explicit plan-v2 maxima, and correlation
identity. It does not reinterpret lowering-v1.

## Diagnostics

Every lowering diagnostic carries a stable code, semantic path, safe message,
optional exact expected type contract, and optional authored origin. It never
carries source value material.

| Code | Meaning |
|---|---|
| `CND-LWR-001` | required node, type, port, or template contract unavailable |
| `CND-LWR-002` | unknown configuration field |
| `CND-LWR-004` | required configuration field absent |
| `CND-LWR-005` | literal kind or value rejected by its expected contract |
| `CND-LWR-006` | integer outside the expected contract bounds |
| `CND-LWR-007` | schema default invalid for its declared contract |
| `CND-LWR-008` | required domain type provider unavailable |
| `CND-LWR-009` | secret reference violates sensitivity or identity boundary |
| `CND-LWR-010` | port-group direction conflicts with the referenced complete port contract |

Duplicate configuration or record fields are rejected earlier by strict source
parsing as `CND-SRC-002`. This preserves the stage that owns the error.

## Conformance

`conformance/c3/source-lowering-v1.json` is normative, language-neutral input.
It declares the exact fixture catalog and literal grammar and covers:

- every version 1 literal form;
- explicit/default equivalence and visible provenance;
- nested records and canonical map ordering;
- signed integer boundaries and overflow;
- precision-sensitive exact values;
- secret redaction and nested-secret rejection;
- imported schemas, imported-default origin, and multi-file spans;
- unknown, duplicate, missing, wrong-type, invalid-default, and unavailable
  provider failures; and
- finite group expansion, resolved port contracts, and exact pool templates.

The hosted Rust consumer executes every case through the public parser, module
resolver, and lowering interfaces. Other implementations consume the same
fixture through the conformance request protocol.

## Normative requirements

| ID | Obligation |
|---|---|
| LWR-001 | Keep every source-to-plan stage explicit and prohibit execution or host probing during lowering |
| LWR-002 | Parse and retain the exact closed version 1 literal forms without binary-float identity |
| LWR-003 | Validate every authored value against an exact node-provided field and type contract |
| LWR-004 | Apply only canonical schema defaults and make their provenance visible |
| LWR-005 | Give explicit-default and omitted-default equivalents one semantic descriptor |
| LWR-006 | Keep authored, schema-default, and plan-binding provenance distinct from semantic identity |
| LWR-007 | Retain exact cross-module source origins separately from lowered identity |
| LWR-008 | Emit stable, value-safe diagnostics with source span and expected contract when available |
| LWR-009 | Retain secret references unresolved, plan-scoped, and redacted from ordinary output |
| LWR-010 | Canonically order unordered values while preserving ordered list semantics and exact precision |
| LWR-011 | Resolve and expand finite port groups to ordinary exact port contracts without live work |
| LWR-012 | Resolve templates and emit exact finite pool specifications without implementation or host selection |
