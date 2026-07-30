# Conduit conformance harness version 1

Status: C2 normative protocol

This document freezes the portable fixture inventory and process protocol used
to compare Conduit implementations. It does not make the Rust harness, Cargo,
or generated output normative. The normative inputs are the reviewed artifacts
under `conformance/`, the versioned manifest, and the semantic specifications
named by their requirement IDs.

## Identity boundaries

The following remain distinct:

- a semantic descriptor or contract;
- a source fixture row or JSON value;
- a request materialized from that fixture;
- an implementation result;
- a harness comparison;
- a resolved execution plan;
- run evidence; and
- presentation of any of those records.

A request ID locates a conformance case. It is not the semantic identity of the
descriptor, plan, event, or diagnostic being tested. A comparison is not run
evidence.

## Versioned inventory

`conformance/v1/manifest.json` is the version 1 inventory. It names:

- the fixture and protocol versions;
- a deterministic clock, seed, and fixed host observations;
- each suite, profile, operation, and normative artifact;
- the SHA-256 digest of every artifact;
- stable requirement IDs for every suite and artifact;
- fields that form the case identity;
- fields that are implementation inputs and expected outputs;
- a default exact rule and reviewed per-case rule overrides;
- positive, negative, boundary, and migration coverage representatives;
- deterministic byte, recursion, and discovery-order property seeds; and
- the Rust reference consumers that execute the fixtures.

Every non-comment TSV row, every NDJSON record, and every member of a declared
JSON vector collection becomes exactly one case. The harness rejects duplicate
case identities, malformed rows, unknown coverage references, digest drift,
empty artifacts, and unsupported manifest protocol versions. No row may be
silently skipped.

The manifest coverage lists identify reviewed representatives of each coverage
class. They do not limit execution: all discovered cases are required.
`default_rule` and `case_rules` reduce the artifact's requirement set to the
single exact rule carried by each materialized request and difference.

## Request protocol

The file protocol is UTF-8 NDJSON. `conduit-conformance requests` writes one
request per line in manifest order:

```json
{
  "protocol_version": 1,
  "fixture_version": "conduit.conformance/v1",
  "request_id": "flow/flow-policy-v1/block",
  "suite": "flow",
  "fixture": "flow-policy-v1#block",
  "profile": "conduit.c2",
  "operation": "bounded-flow-transition-v1",
  "requirement_ids": ["FLW-001", "FLW-002"],
  "result_fields": ["disposition", "events", "outcome", "queue_after", "reason", "state", "transition"],
  "environment": {
    "clock": {"basis": "conduit.fixture-clock/v1", "tick": 1000},
    "seed": 1380991557,
    "host_observations": []
  },
  "input": {}
}
```

The actual `input` contains every source field except the case identity and
expected-result fields. `result_fields` declares the required output shape, but
expected values are deliberately absent. An implementation selects behavior
using `operation`, not repository paths or a Rust-specific type.

The following commands are the stable file interface:

```text
conduit-conformance audit [manifest]
conduit-conformance verify-fixtures [manifest]
conduit-conformance requests [manifest]
conduit-conformance check-results [manifest] < results.ndjson
conduit-conformance reference [manifest]
```

`verify-fixtures` checks artifact digests, manifest coverage, and canonical
reference outputs without launching the manifest's Rust test binaries. CI uses
it after the workspace suite has already run those binaries. `reference`
remains the standalone command that performs both fixture verification and
the complete referenced Rust test set.

With no manifest argument, the version 1 manifest is selected.

## Implementation results

An implementation writes exactly one result for every required request:

```json
{
  "protocol_version": 1,
  "fixture_version": "conduit.conformance/v1",
  "request_id": "flow/flow-policy-v1/block",
  "status": "completed",
  "actual": {}
}
```

`actual` must contain exactly the expected fields for the artifact. JSON
numbers, strings, arrays, objects, booleans, and null compare by JSON value.
Object member order is irrelevant. Extra and missing members are differences.

An implementation that cannot execute a profile or version must report it:

```json
{
  "protocol_version": 1,
  "fixture_version": "conduit.conformance/v1",
  "request_id": "flow/flow-policy-v1/block",
  "status": "unsupported",
  "diagnostics": [
    {
      "code": "implementation/unsupported-profile",
      "message": "conduit.c2 is unavailable"
    }
  ]
}
```

Unsupported is visible and is not a pass. A missing result is a failure.
Unknown or duplicate request IDs are protocol errors.

## Comparison results

`check-results` writes one NDJSON comparison for every manifest case. A
comparison contains the protocol and fixture versions, request ID, exact
fixture ID, governing requirement IDs, status, and structured differences.
Each difference contains:

- `rule`: the exact first governing semantic requirement;
- `path`: an RFC 6901-style JSON pointer;
- `expected`: the normative value; and
- `actual`: the submitted value.

A non-passing comparison makes the command fail. A consumer never needs to
parse human prose to identify the fixture, violated rule, or differing field.

## Rust reference

The hosted `conduit-conformance reference` command:

1. audits and expands every normative artifact;
2. independently encodes and hashes every canonical descriptor JSON vector;
3. runs each declared Rust semantic reference test; and
4. reports the exact suite/test if a reference consumer fails.

The ordinary workspace tests also exercise the portable manifest,
request/result comparison, byte seeds, recursion boundaries, and
discovery-order permutations. CI runs the reference command explicitly.
`conduit-core` does not depend on this hosted crate and remains allocator-free
and `no_std`.

## Determinism and properties

Fixtures MUST NOT read a wall clock, discover hosts, call a network service, or
depend on mutable process state. Operations receive their clock, seed, and host
observations in the request.

Byte and recursion seeds are reviewed inputs, not random snapshots. Discovery
order seeds MUST describe permutations of the same set. The Rust property
checks prove request stability, byte round trips, the accepted depth boundary,
the rejected over-depth boundary, and order-independent seed membership.

## Versioning and review

There are two distinct changes:

- A **fixture correction** retains `fixture_version`, increments
  `manifest_revision`, updates the affected artifact digest, and adds a
  correction entry to `conformance/v1/CHANGELOG.md`. A correction may repair
  malformed representation, an incorrect reference, or explanatory metadata;
  it MUST NOT silently change semantic meaning.
- A **backward-compatible suite addition** may increment `manifest_revision`
  when it adds an independently selectable profile and leaves every existing
  request and expected result unchanged. The changelog records the new expected
  operation version and unsupported-profile migration behavior.
- A **semantic fixture version** creates a new versioned manifest when an
  existing operation, input meaning, expected semantic result, stable reason,
  or governing requirement changes. Its changelog entry records the previous
  and new expected versions plus migration notes.

Every review MUST show the affected requirement IDs, old and new artifact
digests, whether expected output changed, reference results, and migration
impact. Implementation-generated snapshots are proposals only; they become
normative only after review as fixture data.

Version 1 artifacts are retained. A new version does not rewrite the meaning of
an old request.

## Normative requirements

| ID | Obligation |
|---|---|
| CNF-001 | Inventory every normative case exactly once with a stable fixture identity |
| CNF-002 | Carry fixture version, profile, operation, requirements, and deterministic inputs in every request |
| CNF-003 | Require one visible completed or unsupported result for every requested case |
| CNF-004 | Identify every failure by exact fixture, semantic rule, and structured difference |
| CNF-005 | Keep fixture data and protocol normative rather than a language implementation or generated snapshot |
| CNF-006 | Run every normative fixture through the Rust reference in CI |
| CNF-007 | Keep clocks, seeds, host observations, discovery order, and external state deterministic |
| CNF-008 | Distinguish corrections from semantic versions and retain migration history |
| CNF-009 | Preserve descriptors, plans, evidence, protocol records, and presentation as distinct identities |
