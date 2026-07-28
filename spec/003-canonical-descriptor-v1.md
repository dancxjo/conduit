# Canonical descriptor form version 1

Status: stable

Canonical form version: 1

Semantic hash algorithm: SHA-256

## Purpose

This specification gives encoding-independent descriptor values one exact byte
form and semantic identity. It freezes the rules implemented by
`conduit-core` and the independent Python reader in `conformance/c1/`.

Canonical descriptor bytes are not `.panel` source, an execution plan encoding,
bytecode, an ELF file, or a package. A schema-aware lowerer first decides which
facts are semantic; the canonical encoder then represents those facts
deterministically.

## Descriptor envelope

A version 1 descriptor is the following concatenation:

| Field | Encoding |
|---|---|
| magic | bytes `43 4e 44 01` (`CND` followed by version `1`) |
| kind | canonical identifier value |
| schema version | unsigned 32-bit big-endian integer |
| body | one canonical value |

The schema version is an exact revision scoped to the descriptor kind. It is
not the canonical-form version and does not imply compatibility.

## Abstract values

All lengths and collection counts are unsigned 64-bit big-endian integers.
Tags are one byte.

| Value | Tag | Payload |
|---|---:|---|
| null | `00` | none |
| false | `01` | none |
| true | `02` | none |
| integer | `10` | signed 128-bit two's-complement, big-endian |
| bytes | `20` | length, then exact bytes |
| UTF-8 text | `21` | byte length, then exact valid UTF-8 |
| identifier | `22` | byte length, then ASCII identifier |
| list | `30` | count, then values in declared order |
| map | `31` | included count, then key/value pairs in canonical-key order |
| set | `32` | count, then members in canonical-byte order |

Version 1 has no floating-point value. A schema may define a decimal, unit
quantity, or floating-point contract and lower it to an exact integer, text, or
bytes representation. It MUST NOT depend on platform floating-point spelling.

Source spellings of the same integer, boolean, escape sequence, or identifier
disappear during schema lowering. Integers that lower to the same mathematical
signed 128-bit value therefore encode identically.

UTF-8 text is not Unicode-normalized by the canonical encoder. NFC, NFD, case,
locale, and confusable handling can change domain meaning and belong to the
owning schema. For example, precomposed `é` and `e` plus a combining acute
remain distinct unless the schema explicitly normalizes them before encoding.

## Identifiers

Identifiers are ASCII. A complete identifier:

- begins with a lowercase letter;
- otherwise contains lowercase letters, digits, `_`, `-`, or `.`;
- may contain at most one `/` namespace separator;
- has a lowercase letter immediately after `.` or `/`;
- does not end with `.` or `/`.

Invalid identifiers are rejected rather than encoded.

## Maps, sets, defaults, and annotations

Map keys are identifiers. The encoder:

1. rejects duplicate keys, including duplicate excluded fields;
2. determines field participation from the descriptor schema;
3. omits annotations;
4. omits a defaulted field when its value is canonically equal to the declared
   schema default;
5. sorts included entries by the complete canonical bytes of the identifier
   key; and
6. emits the included count and sorted key/value pairs.

The canonical bytes of an identifier include its tag and length. Consequently,
shorter identifiers sort before longer identifiers; equal-length identifiers
sort by ASCII bytes.

Sets reject canonically equal duplicate members and sort remaining members by
their complete canonical bytes. Lists preserve order. Reordering source map
fields or source set members cannot change canonical bytes or semantic hashes.

Defaults are schema facts. An encoder MUST NOT invent or infer a default. A
defaulted field whose value differs from the declared default participates like
an ordinary semantic field.

Prose, display labels, comments, source spans, formatting, layout, editor state,
build timestamps, and presentation metadata are annotations and do not
participate. A schema MUST classify them explicitly; names alone do not make a
field non-semantic.

Unknown critical fields MUST be rejected by the schema-aware reader before
canonicalization. A registered extension field is either semantic and
preserved, defaulted according to its schema, or explicitly an annotation.
Silently discarding an unknown field is forbidden. A reader that can preserve
and hash already-canonical bytes but cannot interpret a field may claim
descriptor-reader behavior, not semantic-validator behavior for that schema.

## Bounded portable behavior

The reference encoder:

- streams to a caller-provided sink;
- requires no allocator and no `std`;
- sorts borrowed maps and sets without scratch allocation;
- rejects collection nesting deeper than 64; and
- rejects lengths that cannot fit in the version 1 unsigned 64-bit length.

The depth limit is part of version 1 portable acceptance behavior. Hosted
writers MUST reject the same over-depth value rather than emit bytes a portable
reader is required to reject.

## Semantic hash

The semantic hash is:

```text
SHA-256(
    UTF-8("conduit.semantic-hash/v1") ||
    00 ||
    canonical_descriptor_bytes
)
```

Its external form is lowercase:

```text
sha256:<64 hexadecimal digits>
```

The domain separator prevents bytes used for another purpose from being
mistaken for a Conduit semantic hash. The canonical header and hash domain both
identify version 1 deliberately.

## Errors

Canonicalization rejects:

- invalid identifiers;
- duplicate map keys;
- duplicate canonical set members;
- lengths outside the version 1 range;
- collection nesting deeper than 64; and
- sink failure.

Descriptor-schema validation remains responsible for unknown fields, required
fields, field types, value ranges, and other semantic constraints.

## Frozen vectors and independent reader

`conformance/c1/canonical-v1.json` is normative. It covers:

- reordering map fields;
- reordering set members;
- omitted schema defaults;
- excluded annotations;
- every version 1 scalar category;
- exact UTF-8 behavior;
- semantic changes producing different bytes and hashes; and
- semantic SHA-256 hashes.

`conduit-core` tests those bytes and hashes. The standard-library-only Python
program `conformance/c1/verify_canonical_v1.py` independently writes, reads,
round-trips, and verifies the same vectors.

## Evolution and migration

Version 1 is immutable. A future canonical form:

- uses a new magic version;
- uses a new semantic-hash domain separator;
- publishes new frozen vectors and at least one independent reader;
- does not reinterpret version 1 bytes; and
- documents a schema-aware migration from decoded old abstract values to new
  abstract values.

Migration is not byte rewriting. It decodes under the old rules, validates and
lowers under the target schema and canonical-form rules, and emits a new
identity. Plans and evidence retain the exact old semantic hashes that
influenced them. Equal meaning across canonical-form versions is a separate,
directional compatibility claim rather than hash equality.

## Normative requirements

| ID | Obligation |
|---|---|
| CAN-001 | Emit the exact version 1 descriptor envelope and value tags |
| CAN-002 | Normalize map and set order without changing list order |
| CAN-003 | Omit only schema-declared equal defaults and annotations |
| CAN-004 | Reject duplicate, invalid, over-depth, and over-length values |
| CAN-005 | Preserve exact UTF-8 unless the owning schema normalizes it |
| CAN-006 | Hash the domain separator and complete canonical descriptor bytes |
| CAN-007 | Never silently discard unknown fields |
| CAN-008 | Keep source and presentation facts outside semantic identity |
| CAN-009 | Preserve version 1 forever and migrate through decoded semantics |
