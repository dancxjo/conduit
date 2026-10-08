# Exact typed pairing preparation storage

The existing structured composer and typed tuple pair encoder now expose
allocation-free storage reservations, separately bounded constructors and actual
retained requested-capacity inventories. Existing constructors and canonical
admission/encoding guards remain available. The pair bounded constructor borrows
both original Types, charging its retained clones before constructing them.

Composer construction explicitly reserves the exact record/case array instead
of relying on a collecting iterator's spare capacity. Its reservation counts
Type prefix, output, array slots, every field/case name and child canonical Type
buffer. The bounded pair constructor retains the exact canonical anonymous tuple
identity. It hashes each original Type through the existing semantic digest,
uses explicitly reserved tuple identity and Kind strings, and calls the original
record constructor and full aggregate traversal guards. It avoids temporary
profile Type clones and formatting buffers. Preparation additionally charges
both child digest encodings and the record constructor's canonical validation
buffer; retained inventory includes tuple Type children, schemas, composers and
all output buffers. Source spare capacities conservatively overcount cloning.

Preparation bounds count cumulative requested allocation bytes, which also
bounds requested live peak. Retained bounds count requested payload capacities.
Root inline values, allocator metadata, stack, shared caller-owned Types and
unrelated runtime preparation are excluded. No global runtime limit increases.
No complete FARGAN driver or whole working-memory admission follows; a caller
must explicitly select the bounded entrance and compose all remaining owners.

Actual allocator gates cover six Type shapes and 36 left/right combinations,
exact canonical Type/maximum parity, both one-under ceilings refusing with zero
allocation, and allocation-free primitive/nominal runtime encoding with original
malformed/schema refusal parity. Composer leaf/record/variant actual cumulative
requests equal retained bytes (4119/4242/4243 in the tested 4096-byte profiles).
All 88 Core unit tests, five existing typed pair tests, two validator allocator
tests and three new composition allocator tests pass. Core all-target Clippy and
no-default-features library checking pass. Evidence is retained in project
outputs/fargan-pair-preparation-storage.
