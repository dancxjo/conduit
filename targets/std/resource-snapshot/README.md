# Linux immutable snapshot file effects

`SnapshotDirectory` retains an actual Root-supplied directory descriptor.
`PreparedFileSnapshot` prepares one exact reference using the existing shared
record codec, two bounded derived names and a finite read buffer. Retained binding
authority is checked before external I/O. Operations resolve no caller-provided
paths, and reads refuse symlinks, special/oversized files and corrupt records.
Directory ownership must exclude competing mutators; this is cooperative Root
file custody, not hostile-code confinement.

Publication exclusively creates a candidate, writes and syncs its record, creates
a non-replacing published hard link, removes the candidate and syncs the directory.
Success follows all those steps. Every attempted write is terminal for that
preparation. Existing generations, abandoned candidates and uncertain I/O stages
remain distinct. Nothing retries, removes abandoned work or interprets an occupied
published name as success. Root must explicitly reconcile uncertain outcomes.

Run `cargo xtask check linux-snapshot`. Six tests use actual local file effects,
including fresh-Boot reopen, duplicate preservation, authority/extent preflight,
symlink/FIFO/oversize/corruption refusal, abandoned candidates and directory rename.
Successful prepared publication and read allocate no Rust heap. The check also
runs shared codec tests and strict all-target package lint. Crash/power-loss
acceptance is not established by OS sync success.

This standalone preparation package creates no Host offer, thread, retry loop,
kernel scheduler, catalog seed or Body. Root still has to bind the directory to
its selected resource residence, reserve disk capacity including candidates,
admit live Host/Boot/Play capability custody and complete memory/work bounds, and
connect the typed snapshot operation to the actual kernel request. Workspace/Host
integration and the Pi recorder service remain subsequent work. Refs #6352.
