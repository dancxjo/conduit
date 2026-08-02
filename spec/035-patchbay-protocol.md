# Patchbay authoring and observation protocol current form

Status: candidate normative C8 contract.

Patchbay is a transport-neutral authoring and observation protocol over the
same `.panel` source, descriptors, exact plans, and Resonance evidence used by
every other Conduit surface. It does not define a graph language, runtime, or
event store.

## Separate resource identities

`SourceDocument` retains exact editable source and a monotonic source revision.
Its parsed source semantic hash is distinct from an optional resolved
descriptor identity, an exact `ExecutionPlan` identity, a `Run` identity, and
an `EvidenceCursor`. `PresentationDocument` stores only node positions,
Use/Build/Inspect mode, structural lens, Logical/Expanded projection choice,
parser/projector-authored selection paths, collapse state, and bounded viewport
metadata. Its hash excludes all semantic and run facts. Moving a box or
navigating a lens changes only presentation revision and identity. The complete
lens contract is in `081-patchbay-structural-lenses.md`.

Runs pin their exact plan and source semantic hash at admission. A later source
or presentation edit cannot mutate that plan or reinterpret its evidence.

## Transactions

Every request names protocol version, source document, expected source
revision, expected presentation revision, and ordered operations. The current
reference operations are source replacement and presentation-only node move.
The server parses source through `conduit-panel`, validates visual paths against
that parsed source, and applies all operations or none. Presentation navigation,
selection, collapse, and viewport operations cannot resolve, grant, acquire,
edit source, transition a plan, or execute. Version mismatch, stale bases,
malformed source, absent visual subjects, and client-invented selection paths
are structured, stable failures.

## Observation

Snapshots and deltas name an explicit logical or expanded subject path and a
shared typed evidence cursor. Patchbay projections are bounded, rebuildable
views; a dropped cursor range produces a `Gap` and requires resynchronization.
Clients MUST NOT infer missing deltas. Patchbay never appends executor evidence
or exposes redacted values merely because it can render a projection.

## Conformance

`conformance/c8/patchbay-protocol.json` covers presentation-only identity,
source identity, atomic conflict and diagnostic behavior, pinned runs,
logical/expanded addressing, explicit resync, and version rejection.
