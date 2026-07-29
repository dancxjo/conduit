# Exact compilation and heterogeneous packages version 1

Status: normative C5 contract. Requirement identifiers `CMP-001`
through `CMP-013` and `PKG-001` through `PKG-018` are exercised by
`conformance/c5/compile-package-v1.json` and
`conformance/c5/compile-source-limits-v1.json`.

## Identity boundaries

Compilation consumes source, the selected root, canonical lowered semantic
descriptors, implementation and artifact manifests, immutable fresh host
reports, grants, resolver policy, and exact finite budgets. It produces one
complete validated `ExecutionPlan` (`CMP-001`). Those inputs retain their own
identities. Compilation does not turn source, manifests, observations,
artifacts, evidence, or presentation into the plan.

A package is a content-addressed envelope. Its manifest identity covers the
ordered versioned manifest projection, while each object retains its own
content digest and optional semantic identity (`PKG-001`). Package identity
does not replace source, module-lock, semantic descriptor, exact-plan,
implementation-manifest, artifact, evidence, or presentation identity.

An exact plan is not generated code. A package is not universal bytecode, ELF,
a container, an executor, or a promise that every contained implementation
can run on every host (`CMP-002`, `PKG-002`).

## Explicit compile input

`conduct compile` accepts one `.panel` entry and one versioned compile-input
document. Compile-input schema 2 pins:

- the entry URI, selected root, and complete content-addressed module closure;
- exact maximum entry bytes, per-module bytes, aggregate closure bytes, and
  module count;
- one identity-bound finite semantic node, port, and type catalog snapshot
  used during lowering (a missing or mismatched pin fails closed);
- implementation and artifact manifests;
- the full identity-checked bounded execution profile pinned by each selected
  implementation manifest;
- immutable host capability reports and their freshness observations;
- grants and explicit authority decisions;
- resolver descriptor, policy, named time basis, and exact resolution tick;
- complete per-node and aggregate resource, memory, queue, authority,
  evidence, and transition budgets; and
- finite resolver and document limits.

The compiler performs parse, module resolution, validation, lowering, host
resolution, budget binding, exact-plan construction, plan sealing, and
portable plan validation in that order (`CMP-003`). It performs no network
discovery, artifact download, host provisioning, login, secret resolution,
grant acquisition, implementation loading, or node execution (`CMP-004`).
Every provider call is an explicit in-memory lookup over the supplied
document.

The compile-input JSON document itself is read with the exported 16 MiB
implementation ceiling. After decoding only that bounded document, the
selected source limits are checked against the exported implementation
ceilings before any entry source is read or module source is parsed. Both file
and stdin entries retain at most the selected maximum plus one detection byte.
Oversized input is rejected as `CND-CMP-009`; it is never truncated into a
different accepted source. UTF-8 conversion occurs only after this bounded
read. The explicit module closure is then checked for count, each source
length, entry length, and checked aggregate length before module parsing
(`CMP-013`).

`using ready` remains a constraint. It is accepted only when a supplied fresh
report and exact manifest satisfy it under the supplied policy. No ready
implementation is inferred from ambient registry state (`CMP-005`).

The result uses the finite `conduit.result/v1` stdout envelope. Human status
and every diagnostic use stderr. A failed compile writes no partial plan to
stdout (`CMP-006`). Identical explicit inputs produce byte-identical plan JSON
and the same plan identity independent of JSON object order, manifest order,
filesystem order, or wall clock (`CMP-007`).

The compiler rejects unresolved selectors, missing or incompatible
implementations/artifacts, absent or stale reports, denied authority,
unsupported versions, arithmetic overflow, and any budget violation
(`CMP-008`). A successful result carries a full execution profile for every
primitive, round-trips through the schema-3 exact-plan decoder, and passes
portable validation without provider access or execution (`CMP-009`).

Compile-input schema 2 retains the schema-1 planning model, and its
`conduit.execution-plan/v3` document predates the live distributed-session
requirements in specification 037. This workflow
therefore rejects a placement whose cord endpoints select different hosts
instead of emitting an older plan with hidden transport semantics. A schema-9
planner must supply one exact `PlanDistributedCord` per cross-host cord; a
future compile-input migration may expose those fields without reinterpreting
v1 input.

Compile-input schema 1 is not reinterpreted with ambient source limits.
Producers migrate by selecting all four explicit source limits, changing the
schema marker/version to `conduit.compile-input/v2` / 2, and resealing the
compile-input identity. This does not change the canonical primary `conduct`
invocation or execution-plan document schema.

The additive `conduit.execution-plan/v7` document carries plan-v15 typed
supervision bindings from specification 049. Compile input must supply one
exact action/resource/timer binding for every lowered grammar-v2 supervision
relationship. Missing, extra, incompatible, or underallocated bindings fail
before a partial plan is emitted; older documents remain readable under their
frozen plan versions.

## Package manifest

`conduit.package/v1` contains:

- schema and package-manifest identity;
- a finite canonical object table keyed by SHA-256 content digest;
- byte size, media type, role, and embedded/thin disposition per object;
- an optional original object identity with kind and schema version;
- license expressions and content-addressed license objects;
- content-addressed SBOM, signature, attestation, provenance-source, and build
  recipe references; and
- optional non-executing retrieval hints for thin objects.

Objects can represent source/module locks, semantic descriptors, plans,
implementation manifests, Linux native artifacts, WASM/components,
process/FFI artifacts, model/data blobs, embedded firmware, licenses, SBOMs,
signatures, attestations, migrations, examples, and presentation assets
without granting them universal execution semantics (`PKG-003`).

Every digest is unique, every size is nonzero, referenced metadata objects
exist with the required role, and every embedded object has exactly one
matching blob (`PKG-004`). Thin objects have no embedded blob and retain an
explicit retrieval hint. Retrieval is never performed by package creation,
validation, inspection, or extraction (`PKG-005`).

## Binary envelope and extraction

The v1 binary envelope is deliberately pathless:

1. the eight-byte magic `CNDPKG1\n`;
2. a big-endian unsigned 32-bit manifest byte length;
3. UTF-8 manifest JSON;
4. a big-endian unsigned 32-bit embedded-object count; and
5. for each digest-sorted object: 32 digest bytes, a big-endian unsigned
   64-bit blob length, and exactly that many blob bytes.

There are no archive entry names, symlinks, permissions, devices, owners, or
timestamps (`PKG-006`). Extraction derives the only output path from the
validated lowercase digest:
`blobs/sha256/<64 hexadecimal digits>`. A manifest string can therefore never
select an extraction path (`PKG-007`).

Decoding checks the complete package byte limit, manifest byte limit, object
count, per-object size, aggregate extracted size, checked offset arithmetic,
duplicate entries, trailing bytes, declared size, and SHA-256 before exposing
an object (`PKG-008`). Extraction uses exclusive file creation beneath an
explicit output directory and never follows package-supplied paths
(`PKG-009`).

Inspection parses and validates only the envelope and metadata. It never
executes, loads, imports, dynamically links, maps as WASM, starts a process,
flashes firmware, retrieves a URI, or asks an implementation to describe
itself (`PKG-010`).

## Integrity and trust

Content digest and size are mandatory integrity checks. License, SBOM,
signature, attestation, and provenance references are distinct metadata
relationships and cannot be inferred from a digest (`PKG-011`). A trust
policy may require known licenses, an SBOM, signatures, attestations, or
provenance for selected roles. `conduct package verify` accepts that bounded
policy plus a bounded JSON array of external cryptographic verification
observations. Each observation identifies the target object, declared
signature, trusted signer, scheme, verifier, and a declared content-addressed
evidence receipt. The package never treats a signature blob or signed code as
trusted merely because it is present (`PKG-012`).

Unsupported package, plan, or Conduit media versions fail closed. Unknown
general media types remain describable data, but cannot be treated as a known
Conduit plan, manifest, or executable kind (`PKG-013`). Error messages report
bounded stable codes and never reflect arbitrary hostile blob bytes
(`PKG-014`).

## CLI compatibility

The canonical primary invocation remains:

```text
conduct [--check|--explain|--run] [PANEL|-]
```

`compile`, `package`, and `inspect` are additive reserved secondary
operations. An ordinary path equal to a reserved word remains addressable
after `--` (`CMP-010`). Compile and package operations support human or finite
JSON output, keep diagnostics independently encoded, and reject NDJSON
(`CMP-011`).

## Stable reasons

- `CND-CMP-001` unsupported compile-input schema
- `CND-CMP-002` compile-input identity or descriptor invalid
- `CND-CMP-003` module/source closure invalid
- `CND-CMP-004` semantic lowering failed
- `CND-CMP-005` unresolved selector
- `CND-CMP-006` implementation, artifact, or host resolution failed
- `CND-CMP-007` resource, queue, authority, or transition budget failed
- `CND-CMP-008` exact plan construction or portable validation failed
- `CND-CMP-009` entry source or explicit module closure limit exceeded
- `CND-PKG-001` unsupported package schema or media version
- `CND-PKG-002` package-manifest identity mismatch
- `CND-PKG-003` malformed or duplicate object metadata
- `CND-PKG-004` missing or unexpected embedded blob
- `CND-PKG-005` object digest or size mismatch
- `CND-PKG-006` missing license, SBOM, signature, attestation, or provenance
- `CND-PKG-007` package or extraction limit exceeded
- `CND-PKG-008` malformed or truncated binary envelope
- `CND-PKG-009` unsafe or conflicting extraction target

## Normative requirements

| ID | Obligation |
|---|---|
| CMP-001 | Emit only the existing complete exact `ExecutionPlan` identity |
| CMP-002 | Keep compilation distinct from code generation and execution |
| CMP-003 | Parse, lower, resolve, bind, seal, and validate in fixed order |
| CMP-004 | Perform no fetch, provisioning, login, grant acquisition, load, or execution |
| CMP-005 | Resolve `using ready` only from explicit fresh inputs |
| CMP-006 | Keep finite results on stdout and diagnostics on stderr |
| CMP-007 | Produce deterministic bytes from identical explicit inputs |
| CMP-008 | Reject every unresolved, stale, incompatible, unauthorized, or over-budget input |
| CMP-009 | Round-trip successful plan output through portable validation |
| CMP-010 | Preserve canonical primary CLI and `--` path disambiguation |
| CMP-011 | Use human or finite JSON, never NDJSON, for compile/package |
| CMP-012 | Bound every input, provider lookup, collection, and diagnostic |
| PKG-001 | Keep envelope identity and every contained identity distinct |
| PKG-002 | Claim no universal execution semantics |
| PKG-003 | Represent heterogeneous object roles without privileged runtimes |
| PKG-004 | Require unique exact object metadata and matching embedded blobs |
| PKG-005 | Never retrieve thin objects implicitly |
| PKG-006 | Use a pathless, timestamp-free deterministic binary envelope |
| PKG-007 | Derive extraction paths only from validated digests |
| PKG-008 | Enforce checked finite decode and extraction limits |
| PKG-009 | Create extracted objects exclusively beneath the caller target |
| PKG-010 | Inspect without executing or loading objects |
| PKG-011 | Keep digest, license, SBOM, signature, and provenance claims distinct |
| PKG-012 | Require explicit trust observations; presence is not verification |
| PKG-013 | Reject unsupported Conduit media and schema versions |
| PKG-014 | Emit bounded stable diagnostics without hostile bytes |
| PKG-015 | Support manifest-only thin and embedded thick envelopes |
| PKG-016 | Preserve object identities through encode/decode/extract round trips |
| PKG-017 | Reject traversal, duplicate, truncated, oversized, and trailing-input attacks |
| PKG-018 | Perform no artifact execution during create, validate, inspect, or extract |
