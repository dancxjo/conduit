# body lifecycle architectural waists

This document freezes the shared contracts implemented by `conduit-body`,
`conduit-core`, and `conduit-kernel`. Product surfaces project their evidence;
they do not own mutation, permission, scheduling, or supervisory truth.

## Durable body continuity

A body remains the same body while at least one admitted part retains a valid,
bounded continuity claim. The minimum payload is the exact body and part
identities, membership generation and proof, workload revision, and durable
authority or revocation facts required by that membership protocol. host,
boot, offers, resources, lines, plans, and plays are current realization truth
and are not continuity payload. Reboot replaces boot truth and clears stale
plan/resource/line/authority bindings. Replacement machinery joins through a
fresh admitted part claim. After the last claim is destroyed, Conduit promises
neither recovery nor fabrication of the same identity. `SOUL` names only this
bounded continuity material and protocol.

## Atomic workload replacement

One serialized transaction performs `propose -> check -> plan -> reserve ->
prepare -> quiesce -> commit -> start`. A commit publishes the workload
revision and immutable plan together; play identity remains distinct and may
be absent in a truthful LULLED result. Add, remove, change, and machinery-loss
replacement use this path. Refusal releases attempt-owned reservations and
leaves the previous revision/plan/play immutable and inspectable. Changed
authority, host, boot, line, cancellation failure, play-start failure, or a
competing revision refuses explicitly. Old completions are stale after commit.

## Causal evidence over signs

Finite edges relate exact evidence identities with `derived-from`, `caused-by`,
`requested-by`, `admitted-by`, `refused-by`, `realized-by`, `observed-from`,
`terminated-because`, `supersedes`, or `corrects`. Edges cross exact execution
and host-session boundaries. Corrections append; history is immutable. Missing
or ambiguous edges are unknown, and timestamps never manufacture causality.
Model output remains info even when an edge records its inputs.

## resource collections and acquisition

A typed collection has a finite entry capacity. resource identity, entry
identity, and immutable generation identity remain distinct. Exact-name,
inclusive time-window, relation, and latest selection are deterministic and
admit candidate and result counts. Ambiguous latest, corrupt/missing
generation, collection pressure, candidate pressure, and result pressure are
distinct refusals. This storage-neutral waist serves both retained
session/history and reviewed form/template catalogs; it is not containment,
a filesystem, or a global namespace.

Attended resources share `discoverable -> requested -> current -> released |
revoked | lost`, with denial and pre-acquisition cancellation separate.
Reacquisition creates a fresh generation; stale handles and completions refuse.
Current resource truth may produce a CapabilityOffer, but acquisition grants
neither workload authority nor plan selection. Browser media, WebSerial, and
WebUSB consume these semantics; opening a product never requests permission.

## Administration and fault disposition

Birth, join/leave, form revision, wake/lull, resource acquisition/release,
authority change, inspection, and surviving-part recovery are exact bounded
administrative intents. Mutation checks authority at the trusted boundary;
reachability and membership are not authority. CLI and Patchbay/Creche are
front ends to this same operation and evidence result. Inspection is separate
and read-only. Stale, denied, conflicting, and replayed requests remain distinct.

plans assign an exact failure scope (gear, cord, form, play, host operation,
resource, or line) and one finite disposition: terminate scope, use a checked
degraded alternative, wait for a bounded number of changes, request atomic
replacement realization, or lull. There is no hidden retry or supervisor
tree. Unrelated scopes continue when policy permits; terminated or replaced
work cannot contribute stale completions. Mandatory physical safety may still
inhibit effects at the local enforcement boundary.
