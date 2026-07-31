# Bounded filesystem boundary

Status: current pre-release contract.

## Separation and identity

`fs/read`, `fs/write`, and `fs/watch` are ordinary host boundary nodes.
Semantic source contains an opaque protected `fs/resource` binding and a
separate grant binding. It never contains an operating-system path, file
descriptor, directory iterator, glob, current-directory lookup, or framework
object.

The provider owns the finite mapping from resource identity to host path and
scope root. The exact plan separately pins the semantic contract,
implementation, artifact, host observation, resource, lease, grant, execution
limits, pending-operation limit, queue memory, and evidence budget. Provider
availability is not authority, and authority is not a resource handle.

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
