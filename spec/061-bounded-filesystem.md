# Bounded filesystem boundary

Status: current pre-release contract.

## Separation and identity

`fs/read`, `fs/write`, and `fs/watch` are ordinary host boundary nodes.
`fs/write-result/sink` is an effect-free observer for one exact typed write
result; it owns no filesystem resource or authority.
Semantic source contains an opaque protected `fs/resource` binding and a
separate grant binding. It never contains an operating-system path, file
descriptor, directory iterator, glob, current-directory lookup, or framework
object.

The provider owns the finite mapping from resource identity to host path and
scope root. The exact plan separately pins the semantic contract,
implementation, artifact, host observation, resource, lease, grant, execution
limits, pending-operation limit, queue memory, and evidence budget. Provider
availability is not authority, and authority is not a resource handle.

## Typed selection and protected binding profiles

A task front may name a typed user/site binding slot. That slot identity is
authored semantic configuration; its current value is not. Selection is a
versioned Patchbay request against one fresh provider observation and an exact
binding revision. The provider returns only an opaque handle plus a bounded
safe label. Operating-system permission and a Conduit grant are confirmed as
separate facts. A selected handle without the exact required read or write
grant cannot resolve into a candidate plan.

The current Copy profile has two incompatible slots:

- `conduit.binding/copy/source-file` requires read authority;
- `conduit.binding/copy/destination-file` requires write and replace authority.

The same protected resource cannot fill both slots, even when two resources
have the same safe label. Choosing, replacing, revoking, forgetting, changing
provider generation, or losing a resource increments or invalidates the
protected profile without editing the shared `.panel` source. An active run
continues against its already pinned exact resource, grant, lease, plan, and
epoch; only candidate resolution follows the new binding revision.

Provider profiles remain explicit and unequal. The deterministic provider may
enumerate its finite fixtures. Browser selection exposes only chooser/create
or download ceremonies supported by that browser host. The hosted-local broker
accepts picker-mediated paths inside its configured root, rejects symlinks and
scope escape, and does not enumerate unless the broker was explicitly granted
that operation. An unsupported host reports unsupported rather than presenting
an inert chooser.

## Read

One `fs/read` operation declares:

- an unsigned offset, maximum operation bytes, and maximum chunk bytes;
- snapshot or live consistency;
- terminal EOF behavior; and
- discard-on-cancellation behavior.

The provider emits an `fs/chunk` and an `fs/read-result` containing the byte
count, next offset, provider generation, and EOF state. A read at or beyond EOF
returns an empty chunk and explicit EOF. Both the deterministic oracle and the
Linux provider reject zero, inverted, or provider-exceeding bounds.

## Write

One `fs/write` operation consumes `fs/chunk` and declares:

- create, replace, or append mutation policy;
- a maximum accepted byte count;
- fail-without-commit or report-committed-prefix partial-write behavior;
- no flush claim or provider-accepted flush;
- close cleanup; and
- close-on-cancellation behavior.

`fs/write-result` reports bytes written, provider generation, whether anything
committed, whether the input completed, and the exact flush claim. The current
Linux and deterministic providers reject a durable claim because neither can
prove durable persistence. Replace and append require an existing resource;
create requires an absent resource. No mode implies atomic replacement.

The checked Copy composite explicitly exports `mode` and `maximum_bytes` with
source `bind` declarations into the private writer configuration, and exports
the writer's `result` port. A required child field supplied by such a binding
is deferred while lowering the definition body and remains required on every
composite instance. The result sink keeps that exported semantic value on an
exact observable cord. Its domain status and fields remain distinct from run
terminal, cleanup, and evidence-publication state.

## Watch

One `fs/watch` operation pins the monotonic clock descriptor and hash, supported
create/change/remove/rename event set, initial-snapshot relation, coalescing
policy, explicit-loss policy, queue capacity, maximum emitted events, overflow
behavior, opaque rename identity, and close-on-cancellation behavior.

The deterministic oracle has a fixed event array. Queue overflow becomes an
explicit gap or gap-followed-by-resync outcome; it is never silently dropped.
The Linux provider preserves an opaque handle across rename only when a bounded
scope scan finds the same device/inode identity. Scan overflow, identity loss,
provider loss, symlink substitution, and cancellation are terminal structured
outcomes.

## Host safety and redaction

Provider installation rejects relative paths and bindings outside their exact
scope root. Linux opens use no-follow behavior and reject final symlinks.
Reads are installed by the reference CLI profile; write and watch providers
require explicit enablement. The browser Tour explicitly installs a bounded
in-memory provider and has no host filesystem access.

Sensitive provider mappings project only a redaction marker. Protected source
bindings remain unresolved secret references through lowering; diagnostics and
Tour presentation never expose a mapped path or protected content.

Normal binding inspection and redacted export may contain slot identity,
revision, provider class/state, safe label, permission/grant state, and allowed
operations. They never contain the opaque resource, exact grant, path,
credential, selected bytes, or unrestricted provider metadata. A stricter
export policy refuses the protected artifact entirely. Exact resolution is the
only API which can expose handle and grant material to plan construction.

Cancellation consumes the pending chooser identity. Duplicate requests replay
no second effect. A late callback from a cancelled request, prior binding
revision, different provider observation, different generation, or disappeared
provider fails closed. User cancellation, OS permission denial, Conduit grant
denial, stale binding, resource disappearance, unsupported operation, and plan
rejection remain distinguishable outcomes.

## Conformance

The allocator-free oracle covers bounded range and EOF behavior, write modes,
partial commit, unsupported durability, watch ordering, coalescing, gap/resync,
rename identity, and cancellation. The Linux adapter adds scope, missing and
wrong handle, permission, symlink, rename scan, provider loss, and redaction
coverage. Normalized read and write results agree between the two providers
where their declared guarantees match.

Checked standalone panels are
[`examples/filesystem-read.panel`](../examples/filesystem-read.panel),
[`examples/filesystem-write.panel`](../examples/filesystem-write.panel), and
[`examples/dir-watcher.panel`](../examples/dir-watcher.panel).
[`examples/file-copier.panel`](../examples/file-copier.panel) is the checked
read-to-write composition.
