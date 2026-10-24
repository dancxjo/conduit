# Linux immutable snapshot file effects

`SnapshotDirectory` retains an actual Root-supplied directory descriptor.
`PreparedFileSnapshot` prepares one exact reference with the shared record
codec, two bounded derived names and a finite read buffer. It preflights retained
binding authority before external storage work. The directory cannot be swapped
by passing a different path to an operation; reads refuse symlinks and nonregular
files. These are cooperative Root file effects, not hostile-native confinement.

Publication creates an exclusive candidate, writes and syncs its record, creates
a non-replacing published hard link, removes the candidate and syncs the directory.
Only then does it return success. Any attempted write is terminal for that
preparation. Existing generations, abandoned candidates and I/O stages are
machine-readable; a possibly published outcome is never silently retried or
acknowledged as durable. Root must explicitly reconcile uncertain outcomes and
abandoned candidates. An occupied published name alone proves neither valid
content nor durability; it is reported as `PublishedNameExists`. Read/restore checks the original immutable envelope and
digest. Successful prepared read and write paths allocate no Rust heap.

Run `cargo xtask check linux-snapshot`. Six tests use actual local filesystem
operations: publication/reopen under fresh Boot authority, duplicate preservation,
authority/extent preflight, symlink/FIFO/oversize/corruption refusal, explicit
abandoned candidates and directory rename. Strict all-target Clippy is included.
`cargo xtask check resource-snapshot` separately checks the shared codec and its
existing browser operation consumer after the test-fixture extraction.

This is an effect primitive, with synchronous OS I/O latency. It creates no
thread, retry loop, request scheduler, ready Host offer, catalogue seed or Body.
Root must still bind the actual directory to the selected Resource residence,
reserve disk capacity (including candidates), admit live Host/Boot/Play capability
custody and the complete work/memory closure, and run the typed snapshot operation
that validates the selected content profile before completing kernel requests.
Directory ownership must exclude competing mutators. OS sync success is the
publication boundary here; crash/power-loss acceptance has not been performed.
The Pi service, planned MCU Line and recorder application remain unfinished.
Refs #6352 and #6350.
