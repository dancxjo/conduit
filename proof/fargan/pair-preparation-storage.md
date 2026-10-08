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

The follow-up Composite `FlowZipBack` entrance composes that reservation with
encoder boxing, left/right/candidate buffers and the exact nonleaf transport
identity hash encoding. It checks both ceilings before hashing or allocation,
then checks the original complete transport Kind. Separate finite and feedback
entrances preserve the existing state-return/closure policies. The typed-only
inventory explicitly refuses the unsupported legacy primitive encoder owner.
This does not charge a caller's boxed Back root, selected offers or contracts.

A dependency-only scratch crate avoids Composite's broad catalog development
dependencies. Its two actual allocator/runtime tests cover four primitive/nominal
pair preparations, one-under zero-allocation refusal, foreign Kind refusal and
all three StepBack modes under 1000 blocked output steps each. Canonical output,
normal/finite flush and feedback final-state return are checked with zero runtime
allocations. Composite library and probe all-target Clippy pass. The initially
chosen broad Cargo development graph was stopped when it began unnecessary
Language generation; that interrupted gate is not counted as a pass.

`patches/bounded-target-zip.patch` is a checked additive ConduitOS factory patch
against the separately published original FARGAN branch e37a05164. It preserves
the entire retained selected-offer comparison, makes borrowed port selection
allocation-free, and exposes a Back reservation and explicit bounded construction
including the boxed Back root. Its tiny coherent current-SDK façade includes the
exact original FARGAN catalog Source instead of linking an older catalog ABI.
All eleven catalog/factory/component tests and Clippy pass. One-under and foreign
artifact selection refuse without allocation; the actual dynamic finite/feedback
Backs encode the original canonical pair under pressure without allocation.
This patch awaits integration with its Core/Composite dependency checkpoints;
it is not a compiled complete target crate, offer/catalog preparation admission,
or complete public model driver. Evidence: project
outputs/fargan-target-zip-preparation.
