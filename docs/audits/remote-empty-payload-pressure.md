# Empty remote payloads and atomic item pressure

Development slice for #5231. Remote single-ingress and fan-out admission used
only the maximum available byte count. Zero represented both a full queue and
an empty payload that legitimately needs no bytes. An empty value could pass
preflight on a full backpressured queue; fan-out could enqueue an earlier branch
before a later push refused, and the single path could allocate before refusing.

Both paths now use one shared predicate that checks item pressure independently
of byte allowance. A full non-coalescing queue returns `Full` before allocation,
reference retention, queue mutation, Sign publication or remote sequence advance.
Empty payloads remain valid with item room, and admitted coalescing still
replaces the newest queued value, releasing its references normally. No policy,
retry, scheduler or value representation was added.

Two regression fixtures failed before repair. Current tests verify unchanged
value-slot generations, references, lengths and bytes, unchanged Signs and
remote sequences, and no partial delivery when one fan-out sibling is full.
They also admit empty payloads to empty queues and coalesce full queues down to
one shared zero-byte value without retaining the superseded value. These are
production fixed-kernel tests, not transport or physical-device proof.

The alloc and linear-ready reference suites each pass 175 tests; seven
fixed-only remote-fan-out tests and strict all-target kernel Clippy pass.
Historical Sign reservation benchmarks cover their pinned sources and do not
measure this remote-ingress correction. Parent acceptance remains open.
