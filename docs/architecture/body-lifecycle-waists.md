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
neither recovery nor reconstruction of the same identity. `SOUL` names only this
bounded continuity material and protocol.

## Fulfillment is explicit, not inferred

The generic Body lifecycle already includes a terminal `Fulfilled` state.
An attributed `Fulfill` transition is admitted only while the Body is
`Lulled`, with its own authority grant, exact Sign, and settled-obligation
evidence. It prohibits later Wake or ordinary workload mutation, without
deleting the retained biography.

Optional finite purpose and its evidence-derived readiness are separate from
this administrative operation. An authored Plot's normal semantic completion
does not automatically trigger Fulfillment. Concrete runtimes must also prove
their own cleanup and persistence settlement before claiming end-to-end
Fulfillment. See [purpose and fulfillment](body-purpose-and-fulfillment.md).

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
session/history and reviewed plot/template catalogs; it is not containment,
a filesystem, or a global namespace.

Attended resources share `discoverable -> requested -> current -> released |
revoked | lost`, with denial and pre-acquisition cancellation separate.
Reacquisition creates a fresh generation; stale handles and completions refuse.
Current resource truth may produce a CapabilityOffer, but acquisition grants
neither workload authority nor plan selection. Browser media, WebSerial, and
WebUSB consume these semantics; opening a product never requests permission.

## Body naming

The friendly name is bounded administrative metadata, independent of Body
identity, workload revision, membership, Plan and Play. A rename is an explicit
owner-signed request naming the exact Body, receiving Host/Boot, expected
biography sequence, fresh request nonce, new name and attribution. The trusted adapter supplies the
provisioned owner verification key; neither a request-supplied key nor membership
confers permission. Fulfilled Bodies refuse changes.

The lifecycle session appends `Renamed` evidence and updates the current name
atomically. Stale requests refuse; an exact successful retry or an already-current
name appends nothing. The adapter must commit the resulting biography and any
archive custody before issuing a Root-signed completion. That completion binds
the exact rename record digest, authenticating both names, attribution and owner
key as well as the request. Transport acceptance is not completion. A failed or uncertain write requires reconciliation before retry.

Renamed biographies use `biography-evidence@3`, which older writers reject;
existing v2 evidence retains its original name without synthetic rename events.
Name-history compaction preserves the latest rename in the active window and
moves older records into digest-linked v3 archive segments. Face producers
project the committed name using prepared storage, without changing observations
or substituting a Mask-local label.

## Administration and fault disposition

Birth, rename, join/leave, plot revision, wake/lull, explicit fulfillment, resource acquisition/release,
authority change, inspection, and surviving-part recovery are exact bounded
administrative intents. Mutation checks authority at the trusted boundary;
reachability and membership are not authority. CLI and Patchbay/Creche are
front ends to this same operation and evidence result. Inspection is separate
and read-only. Stale, denied, conflicting, and replayed requests remain distinct.

plans assign an exact failure scope (gear, cord, plot, play, Host Call,
resource, or line) and one finite disposition: terminate scope, use a checked
degraded alternative, wait for a bounded number of changes, request atomic
replacement realization, or lull. There is no hidden retry or supervisor
tree. Unrelated scopes continue when policy permits; terminated or replaced
work cannot contribute stale completions. Mandatory physical safety may still
inhibit effects at the local enforcement boundary.
