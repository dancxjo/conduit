# Panel capsules

Status: current pre-release specification.

## Identity and ownership

A `conduit.panel-capsule` is the bounded, readable authored-program document.
It carries exact `panel 0` source bytes, their byte revision, their semantic
source identity, an optional immutable import lock, explicit artifact
references, and an optional PresentationDocument. The program identity covers
source, lock, and artifact references. Presentation changes the capsule
identity but not the program identity.

The capsule is not an ExecutionPlan, provider observation, site selection,
authority grant, active epoch, or run evidence. Opening, checking, inspecting,
diffing, or unpacking it MUST NOT fetch, install, resolve a host, grant
authority, or execute generated source. Resolution creates a distinct exact
plan. An admitted run pins its plan and source identity; a later candidate
document cannot mutate that active epoch.

## Current logical document

The one current form is canonical JSON with schema `conduit.panel-capsule` and
schema version 0. It contains:

- exact UTF-8 source, a SHA-256 source revision, and the parsed semantic hash;
- optional inline JSON import-lock and presentation documents, each with an
  exact digest and public or restricted sensitivity;
- at most 256 sorted artifact references with a role, SHA-256 digest, byte
  size, media type, license, provenance, sensitivity, acquisition policy, and
  explicit non-executable status; and
- separate program and complete-capsule identities.

Roles distinguish fixtures, data, media, models, providers, profiles, site
bindings, detached conformance results, and detached evidence. Provider and
profile references describe requirements only; they do not claim availability.
Site bindings remain protected referenced artifacts and are omitted from a
portable share unless explicitly selected. Detached conformance and evidence
never enter the authored source identity.

A thin reference uses `never` or `explicit` acquisition and carries no bytes.
A small sealed reference uses `embedded`, lower-case hexadecimal bytes, and an
exact matching digest and size. Total embedded bytes are limited to 1 MiB;
source and each auxiliary document are limited to 1 MiB; the decoded capsule
document is limited to 4 MiB. Secret or executable content cannot be embedded.
Large dependencies remain thin content-addressed references.

## Projections and operations

`conduit-panel::SourceDocument` is the lossless structured projection: it
retains every source byte, CST token, span, parsed semantic AST, and diagnostic.
Patchbay edits the same source and keeps presentation under a separate
identity. A programmatic composition produces a candidate source document;
only ordinary check, resolution, authority, and plan-transition machinery may
admit it.

`conduct capsule pack`, `inspect`, `check`, `unpack`, and `diff` are bounded and
deterministic. Pack never fetches a reference. Unpack writes only fixed names
to a newly created directory and refuses overwrite. Diff reports source,
semantic, lock, artifact, program, presentation, and capsule identity changes.

Container encoding is replaceable and non-semantic. The current command reads
only the canonical JSON document and denies unknown fields, so archive paths,
duplicate entries, symlinks, device entries, permissions, and compression do
not exist in accepted input. A future container reader must define and pass
path traversal, duplicate-entry, decompression, link/device, executable-bit,
count, and byte-bound fixtures before it becomes current; an unrecognized
container is malformed input, not a compatibility fallback.

## Requirements

- **CAP-001:** Source round-trips byte-for-byte through the lossless document
  and capsule projections.
- **CAP-002:** Program identity covers source revision and semantics, import
  lock, and ordered artifact references; presentation is excluded.
- **CAP-003:** Capsule validation is offline and performs no provider, network,
  environment, filesystem-discovery, or authority lookup.
- **CAP-004:** Embedded bytes match their lower-case SHA-256 digest and size,
  stay within bounds, are non-secret, and are non-executable.
- **CAP-005:** Exact plan, live epoch, and evidence identities remain distinct
  and immutable after admission.
- **CAP-006:** Patchbay, CLI, and API derive semantic identity from the same
  `conduit-panel` source projection.
- **CAP-007:** External cords remain owned by the enclosing source; a reusable
  definition never absorbs its instances or their enclosing connections.
- **CAP-008:** Unknown fields, displaced schema forms, noncanonical reference
  order, duplicate digests, malformed source, and identity substitution fail
  closed with `CND-CAP-001` through `CND-CAP-007`.
