# Conduit structured diagnostics current form

Status: C3 normative diagnostic contract

This document freezes a portable diagnostic data model and its hosted JSON,
fix-checking, and terminal presentation. A diagnostic describes a failure or
warning. It is not the semantic contract, source document, fix application,
execution plan, evidence record, or terminal rendering that refers to it.

`conduit-core` carries only borrowed allocator-free data. Hosted ownership,
serde, source bytes, ANSI styling, and rendering live in
`conduit-diagnostics`.

## Data model

Every diagnostic has:

- `schema_version`, exactly `1`;
- stable `CND-*` code and severity (`error`, `warning`, or `note`);
- concise value-safe message;
- optional primary source span;
- zero or more labeled related spans or semantic subjects;
- structured public or redacted arguments;
- notes and optional help;
- zero or more unapplied fixes;
- optional expanded semantic instance path; and
- zero or more causal diagnostic codes.

Codes and structured fields are stable API. Safe prose may improve without
changing the failure class. Arbitrary `Debug`, error-chain text, and provider
errors are not user-facing diagnostic protocols.

## Source spans

A source span contains:

- document identity;
- optional exact `sha256:` content identity;
- zero-based, end-exclusive byte range; and
- one-based start/end line and column for presentation.

Byte ranges remain authoritative, including for a non-UTF-8 document. Line and
column are derived presentation facts. A span cannot reverse either range.
Imported and expanded diagnostics retain separate related locations rather
than flattening several source identities into one synthetic position.

## Subjects, paths, and causality

Related entries label the role of each location or subject, such as `writer
port`, `reader port`, `cord`, `import declaration`, or `first declaration`.
The semantic path identifies the logical or expanded instance when known.
Causal codes retain the lower-level compatibility, authority, flow, or
provider reason without embedding its English display.

For `microphone.audio -> tts.text`, `CND-TYP-001` points primarily at the cord,
relates both port declarations, carries `audio.pcm` and `text.utf8` as public
contract arguments, and retains the exact compatibility cause. It proposes an
adapter only when the caller supplies an explicitly known adapter and edit.
Nothing silently inserts or guesses one.

## Sensitivity

Messages, labels, notes, help, paths, document identities, and public arguments
MUST already be safe for their diagnostic audience. Protected material never
uses the public variant. It uses:

```json
{
  "disposition": "redacted",
  "sensitivity": "secret",
  "value_type": "fixture/token",
  "byte_len": 17
}
```

JSON retains this structure. Human verbose output renders the same argument as
`[REDACTED]`. No renderer receives the protected bytes, so color choice,
verbosity, `Debug`, or JSON serialization cannot reveal them.

## Fixes

A fix has stable ID, concise message, applicability, and a non-empty edit list.
Applicability is:

- `machine-applicable`: the edit is exact if every precondition remains true;
  or
- `maybe-incorrect`: an explicit user decision is still required.

Each edit names a document, zero-based end-exclusive byte range, replacement
text, and exact precondition content hash. Fix checks return applicable, stale
precondition, missing document, or invalid range. Diagnostics never mutate
source themselves. A stale fix remains diagnostic data and MUST NOT be applied.

current form adapters provide actionable fixes for at least these common mistakes:

1. missing cord arrow;
2. unsupported panel grammar version;
3. trailing collection comma;
4. directly authored protected material where a secret reference is required;
5. a port mismatch with an explicitly registered named adapter.

The first three are machine-applicable. Protected binding and adapter choices
remain maybe-incorrect.

## Core and hosted adapters

The allocator-free core validates diagnostic version, code shape, spans,
causes, fixes, ranges, and SHA-256 preconditions. It performs no allocation,
source lookup, JSON, or styling.

The hosted crate provides owned equivalents and explicit adapters for:

- parser errors with byte-derived primary spans;
- module resolution errors with import-chain subjects;
- typed lowering errors with expected contracts and semantic paths;
- runtime resolution and execution errors;
- portable topology/compatibility failures with cord and both endpoint spans;
  and
- exact-plan validation failures with collection and semantic path.

`conduct` routes parser, resolution, and runtime failures through these
adapters. It supports:

```text
--diagnostic-format=human|json
--color=auto|always|never
--verbose-diagnostics
```

Human output is presentation and may be concise or verbose. JSON is the
lossless versioned data record.

## JSON

Compact JSON uses field names and enum spellings in this specification.
Optional empty collections may be omitted; decoding restores them as empty.
Unknown fields, malformed codes/spans/fixes, and unsupported schema versions
fail closed.

Example:

```json
{"schema_version":0,"code":"CND-SRC-001","severity":"error","message":"expected `->`","primary":{"document_id":"stdin","content_hash":"sha256:...","byte_start":19,"byte_end":20,"line":2,"column":12,"end_line":2,"end_column":13}}
```

Object member order is not semantic. The hosted encoder has stable struct order
for reproducible streams and round-trips every structured field.

## Code families

| Family | Boundary |
|---|---|
| `CND-SRC-*` | source, CST, parsing, imports, and names |
| `CND-ID-*` | identity and references |
| `CND-TYP-*` | semantic types and providers |
| `CND-PRT-*` | ports and cardinality |
| `CND-CFG-*` | typed configuration contracts |
| `CND-CMP-*` | composites, exports, and expansion |
| `CND-LWR-*` | source validation/defaulting/lowering |
| `CND-FLW-*` | bounded flow and pressure |
| `CND-LIF-*`, `CND-CAN-*` | lifecycle and cancellation |
| `CND-IMP-*` | implementation satisfaction |
| `CND-HST-*` | host observations and resources |
| `CND-AUT-*` | authority and sensitivity |
| `CND-ART-*` | immutable artifacts |
| `CND-PLN-*` | plan resolution and integrity |
| `CND-RUN-*` | execution |
| `CND-EVD-*` | immutable evidence |
| `CND-TRN-*` | transport and remote protocol |
| `CND-EXT-*` | extension contracts |

New meanings allocate a new code. Existing meanings are not repurposed merely
to improve prose.

This specification allocates `CND-TYP-002` for a compatibility decision that
is indeterminate because its required domain provider is unavailable.

## Conformance

`conformance/c3/diagnostics.json` is normative. Its cases cover:

- single-file parse and multi-file name failures;
- a two-ended port mismatch and exact compatibility cause;
- nested expanded paths and related spans;
- machine-applicable and stale fixes;
- unavailable-provider indeterminacy;
- structural redaction;
- stable JSON;
- exact non-UTF-8 byte offsets; and
- plain and ANSI terminal snapshots in the hosted Rust reference.

## Normative requirements

| ID | Obligation |
|---|---|
| DIA-001 | Carry an explicit schema version, stable code, severity, and safe concise message |
| DIA-002 | Retain authoritative document byte ranges separately from presentation locations |
| DIA-003 | Represent primary, related, semantic-path, and causal relationships structurally |
| DIA-004 | Keep the portable core allocator-free and presentation-independent |
| DIA-005 | Encode and decode every diagnostic field in stable lossless JSON |
| DIA-006 | Render concise and verbose human forms with explicit color policy |
| DIA-007 | Represent protected arguments structurally without protected value bytes |
| DIA-008 | Guard every fix edit with document identity, exact range, and content hash |
| DIA-009 | Check fix freshness without mutating source |
| DIA-010 | Suggest an adapter only when an exact named adapter is supplied |
| DIA-011 | Adapt parser, resolver, compatibility, lowering, planning, and runtime failures |
| DIA-012 | Preserve exact byte offsets even when source bytes are not UTF-8 |
