# Implementation and artifact manifests version 1

Status: candidate normative contract for Conduit C5. Requirement identifiers
`MAN-001` through `MAN-010` and `ART-001` through `ART-010` are exercised by
`conformance/c5/manifests-v1.json`.

## Identity boundaries

`ImplementationManifest` is a canonical, versioned resolver input. It MUST pin
one semantic node contract, a stable implementation ID and version, an
executor kind, an entrypoint/adapter/ABI, an execution profile, required and
provided interface contracts, effects, artifact digests, supported plan and
runtime protocol ranges, replacement/coexistence facts, and any reproducible
build claim (`MAN-001`).

The executor kinds are native in-process, WASM component, general FFI dynamic
library, process, firmware, and remote endpoint (`MAN-002`). Python and every
other language use one of these capability boundaries; they are not semantic
node kinds. Several manifests MAY pin the same semantic contract while
retaining distinct implementation identities (`MAN-003`).

The execution-profile pin carries the bounded work, memory, representation,
cancellation, isolation, and host-operation contract defined by specification
022. Required interfaces and effects expose the narrow host-service
obligations needed by specifications 027 and future backend interfaces
(`MAN-004`). An interface implementation is not a mega-host and OS,
framework, private-key, or library types are not semantic facts.

Replacement support is exactly `cold`, bounded `quiescent`, or bounded
`stateful`; stateful replacement pins its state contract and export/import
limits (`MAN-005`). Coexistence memory is explicit. A manifest MUST NOT imply a
guarantee that its execution profile or foreign dependencies do not provide.

## Immutable artifacts

Each blob has a distinct, canonical, versioned `ArtifactManifest` with a
SHA-256 digest, media type, nonzero byte size, optional target and ABI,
provenance, signatures, SPDX-style license expressions, notice/SBOM/source
references, and related artifacts (`ART-001`). Bundle paths and remote URIs are
retrieval hints: changing a mirror does not change immutable artifact identity
(`ART-002`). Every byte returned through such a hint is nevertheless checked
against the manifest digest.

Artifact replacement does not alter the semantic node contract. It does alter
the implementation identity and any exact `ExecutionPlan` that pins the
artifact (`MAN-006`). A reproducibility claim pins source, recipe, and expected
artifact digests; the expected digest MUST be a required implementation
artifact (`MAN-007`).

## Validation and trust

Descriptor validation is allocator-free, bounded by caller-provided scratch,
and performs no I/O, discovery, loading, signature verification, or execution
(`MAN-008`, `ART-003`). Missing entrypoints, malformed identities, unsupported
version ranges, and absent required artifacts fail closed with stable
`CND-MAN-*` reasons.

Entrypoint name, adapter, ABI, and protocol version are exact implementation
facts; an absent or malformed entrypoint is never inferred from language,
media type, or library metadata (`MAN-009`). All unordered implementation
requirements are canonicalized as a set, while each artifact reference stays
content-addressed and declares whether it is required (`MAN-010`).

A hosted byte gate MUST calculate SHA-256 and check byte size before a loader
can observe an artifact (`ART-004`). Exact target and ABI expectations then
fail closed (`ART-005`). A trust policy separately selects whether a signature,
provenance evidence, known license, or SBOM is required (`ART-006`). Signature
verification is an explicit host observation carrying signer, scheme,
verifier, result, and evidence digest. A signature never implies sandboxing or
semantic compatibility (`ART-007`).

Unknown licensing is represented by an empty license set and can be rejected
by policy (`ART-008`). Transitive notice, SBOM, source, and related-blob
references remain content-addressed (`ART-009`). Safe inspection reports
identity, metadata, licensing, provenance, signatures, locations, and
references without fetching, loading, or executing them (`ART-010`).

## Stable reasons

- `CND-MAN-001` unsupported manifest schema
- `CND-MAN-002` malformed or inconsistent descriptor
- `CND-MAN-003` semantic identity mismatch
- `CND-MAN-004` no required artifact
- `CND-MAN-005` unsupported plan/runtime range
- `CND-ART-002` digest mismatch
- `CND-ART-003` byte-size mismatch
- `CND-ART-004` target mismatch
- `CND-ART-005` unsupported ABI
- `CND-ART-006` required or invalid signature
- `CND-ART-007` required provenance evidence absent
- `CND-ART-008` required known license absent
- `CND-ART-009` required SBOM absent

The resolver of issue #26 consumes these declarations plus fresh host
observations. This specification defines no resolver preference or loader.
