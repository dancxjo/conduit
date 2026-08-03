# Safe multi-artifact inspection current form

Status: C3 normative hosted inspection contract

This document defines read-only, bounded, non-executing inspection over
Conduit artifact kinds whose semantic schemas or hosted encodings already
exist. Inspection validates and reports facts; it never compiles, selects an
implementation, provisions a host, acquires authority, resolves a secret,
loads executable code, invokes a provider, or executes inspected bytes.

## Boundary and identities

`conduit-inspect` is a hosted crate above the allocator-free core. Its
`conduit.inspection` report is a presentation-neutral observation about an
artifact. Report identity never replaces:

- source semantic identity;
- lowered semantic identity;
- exact `ExecutionPlan` identity;
- implementation or artifact identity;
- immutable `ExecutionEvent` identity;
- diagnostic structure;
- conformance fixture identity; or
- the SHA-256 content digest of inspected bytes.

Every report names `kind`, artifact schema/version, content digest when bytes
exist, exact semantic identity when the owned type defines one, validation
status, finite counts/budgets, categorized references, a redacted-field count,
and value-safe notes. Human rendering contains only those facts. It never
reproduces source configuration values, diagnostic messages/arguments, or
evidence payload material.

## Supported adapters

| Kind | Current input boundary | Validation |
|---|---|---|
| panel source | UTF-8 `.panel` bytes or local path | lossless CST/source AST, current source identity, local module graph and pins |
| current lowered source | typed `LoweredSource` | schema pair, semantic/source references, topology counts and aggregate authored bounds |
| execution current plan/current/current | typed `ExecutionPlan` plus explicit validation context | portable exact-plan validation, identity, implementation execution profiles, pins, staleness, budgets and references |
| physical execution arrangement | typed `ResolvedExecutionArrangement` plus its exact plan | distinct arrangement/plan/resolution identities, region placement, lane and wake capacity, commit domains and boundaries |
| hosted lane batch | typed `HostedLaneBatchEvidence` plus its resolved arrangement | provider generation, bounded lane activity, causal overlap, proposal pressure, deterministic commit order and arrangement reference |
| execution evidence current | hosted immutable-event NDJSON | record bounds, owned decoding, core event/stream validation, identity/order/redaction |
| structured diagnostic current | hosted diagnostic JSON | exact owned schema and allocator-free contract validation |
| conformance manifest current | JSON bytes or local path | header/schema, uniqueness, digest syntax, and bounded local referenced-digest verification |
| conformance cases | normative JSON-vector or TSV bytes | current suite/header markers and bounded collection structure |

Lowered source, exact plan, physical execution arrangement, and hosted lane
batch currently have semantic Rust/core schemas but no current standalone
hosted byte codec. The typed adapters inspect them exactly.
`conduct inspect --type=lowered-source` and
`--type=execution-plan` reject ad-hoc JSON marker objects with `CND-INSP-008`
instead of inventing, guessing, or blessing a persisted encoding. A future
owning specification may add a byte codec and adapter without changing the
safety contract here.

Future implementation manifests, host reports, packages, and executable
artifacts are not recognized until their owning schemas land. Native ELF,
WASM, process scripts, firmware, containers, compressed archives, and unknown
binary bytes therefore fail closed; the detector never probes them by
execution.

## CLI compatibility

Inspection is the additive secondary operation:

```text
conduct inspect [--type=TYPE] [--format=human|json] ARTIFACT
```

The canonical run/check/explain invocation remains unchanged. Exact path
`inspect` remains runnable with the ordinary positional escape:

```text
conduct -- inspect
```

`inspect.panel`, `./inspect`, and other ordinary paths are not subcommands.
Combining a secondary operation with `--check`, `--explain`, or `--run` is
`CND-CLI-004`. Inspect is finite, so `--format=ndjson` is `CND-CLI-003`.

Human inspection results use stdout. JSON is one `conduit.result` envelope
whose `operation` is `inspect` and whose `result` is one
`conduit.inspection` report. Human or diagnostic JSON failures use
stderr and leave stdout empty. Quiet, general verbosity, color, TTY, broken
pipe, and output-failure behavior remain specifications 018 and 020.

The generated completion model contains `inspect`, every explicit type, all
shared presentation flags, and its artifact positional. Both `conduct(1)` and
`conduct-inspect(1)` are generated from that same model. Inspection performs
no discovery while generating or completing.

## Marker-only detection

Explicit `--type` is authoritative only when it agrees with a current marker.
Auto-detection considers:

- the grammar `panel VERSION` marker after blank lines/comments;
- exact diagnostic object fields;
- exact execution-event record fields;
- conformance manifest version fields;
- conformance suite markers; and
- explicitly reserved lowered-source/plan schema markers.

File extension is only a hint. `.panel` and `.ndjson` conflicts are rejected.
Unknown input is `CND-INSP-001`; multiple valid markers are
`CND-INSP-002`; explicit/extension conflicts are `CND-INSP-003`.
Unsupported versions fail with the owning schema diagnostic where available
or `CND-INSP-004`. Malformed/truncated structure is `CND-INSP-006`.
Detection never consults a network, registry, provider, plugin, host report,
dynamic loader, file executable bit, or behavior of the bytes.

## Fixed bounds and local references

current form defaults are:

| Limit | Value |
|---|---:|
| one input/module/reference | 8 MiB |
| one NDJSON record | 1 MiB |
| records | 4,096 |
| JSON nesting | 64 |
| aggregate JSON structural items | 16,384 |
| local modules | 256 |
| aggregate local module bytes | 32 MiB |
| aggregate conformance reference bytes | 64 MiB |

Reads use a `limit + 1` stream cap before retaining input. JSON receives a
string-aware structural preflight before allocation into a value tree.
Evidence receives byte/count/depth checks before owned decoding.

Local panel imports are confined to the entry file's directory, use the
existing deterministic module resolver, and cannot select URI/network
loaders. Local conformance references are confined to their conformance root,
read within per-file and aggregate limits, and checked against manifest
digests. There is no decompression or archive extraction in current form.
Limit failure is `CND-INSP-005` for the top-level byte ceiling and
`CND-INSP-007` for structural/aggregate ceilings. Existing source/evidence/plan
diagnostics remain authoritative after their decoder boundary.

## Redaction and non-execution

Source secret references are counted but never formatted. Diagnostic messages,
public/redacted argument values, fixes, and source snippets are not copied
into reports. Evidence payload bytes, protected reference digests, redacted
shape, and reasons are validated but not copied into reports. General `-v`,
`--verbose-diagnostics`, human output, JSON output, and debug formatting do
not widen this boundary.

Plan inspection calls the exact portable validator and reports selected pins;
it does not open an artifact or assert executability. Source inspection
reports `using ready` and other unresolved selectors without catalog/provider
resolution. Conformance inspection verifies data and digests without invoking
reference tests.

## Diagnostics

| Code | Meaning |
|---|---|
| `CND-INSP-001` | no supported current marker |
| `CND-INSP-002` | conflicting/polyglot markers |
| `CND-INSP-003` | explicit type or extension conflicts with marker |
| `CND-INSP-004` | unsupported inspection-level version |
| `CND-INSP-005` | top-level input byte ceiling exceeded |
| `CND-INSP-006` | malformed, truncated, invalid-reference, or digest failure |
| `CND-INSP-007` | record, depth, item, module, or aggregate bound exceeded |
| `CND-INSP-008` | semantic type has no current standalone hosted byte encoding |

Owned source, lowering, plan, evidence, diagnostic, and I/O codes pass through
when their owning validator gives a more precise reason.

## Conformance and compatibility

`conformance/c3/inspection.json` freezes 50 positive, negative, boundary,
migration, malicious-input, typed-adapter, and CLI cases. Exact generated help,
completions, root manual, and inspect manual remain drift checked in hosted CI.

Adding a new adapter is compatible only when its owning schema supplies an
unambiguous non-behavioral marker, bounded decoder, version/failure policy, and
redaction rules. New detection must not cause previously unknown/polyglot
bytes to be guessed as executable or collapse identity categories.

## Normative requirements

| ID | Obligation |
|---|---|
| INSP-001 | Keep inspection hosted, read-only, non-executing, and free of implicit network/provider/authority behavior |
| INSP-002 | Preserve source, lowering, plan, physical arrangement, provider observation, artifact, evidence, diagnostic, conformance, content, and report identities separately |
| INSP-003 | Detect only current markers and fail closed on unknown, ambiguous, conflicting, or unsupported input |
| INSP-004 | Apply fixed pre-allocation byte, record, depth, item, module, and aggregate limits |
| INSP-005 | Validate panel CST/AST/source identity and bounded local module graphs without resolving providers |
| INSP-006 | Validate typed lowered sources and exact plans through their owning schema boundaries |
| INSP-007 | Validate complete evidence streams and diagnostics through their existing owned/core contracts |
| INSP-008 | Validate conformance manifest structure and bounded local referenced digests without running tests |
| INSP-009 | Never reproduce secret config, diagnostic value material, or evidence payload material in any report or verbosity mode |
| INSP-010 | Report exact categorized references, finite counts/budgets, content digest, version, and validation outcome |
| INSP-011 | Keep human/result stdout distinct from human/diagnostic stderr |
| INSP-012 | Preserve the canonical run/check/explain path and provide an explicit path escape for the additive secondary operation |
| INSP-013 | Treat broken stdout pipes as success and other output failures as `CND-IO-002` |
| INSP-014 | Reject ad-hoc plan/lowering byte encodings rather than inventing persisted schema |
| INSP-015 | Generate inspect help, completions, and manual pages from the shared Clap command model |
