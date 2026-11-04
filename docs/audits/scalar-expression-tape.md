# Bounded fixed scalar tape

## Resident instruction images (development)

`generate_static_scalar_expression` performs the existing checked fixed-scalar
preparation on the authoring Host and emits Rust static data. The generated
module retains one instruction array, deduplicated literal slices and the exact
canonical program's SHA-256. Its constructor creates a `StaticScalarExpression`
that borrows those instructions; it neither decodes a Source program nor
allocates instruction or result buffers. Evaluation uses the same scalar tape
loop and checked arithmetic as the owned evaluator, writing into caller-owned
bounded output. The digest correlates code with its program; it grants no
execution authority and is not a Source-document identity.

Before execution, the resident profile checks fixed integer/Bool Types, literal
geometry, operand/destination bounds, forward-only control flow, definite slot
initialization on every path and consumption by Move. Unretained proof hints
are refused. The maximum remains 128 instructions. Initialization uses a
129-byte forward-state array and at most quadratic bounded analysis; execution
uses the existing 4/8/32/128-slot scratch profiles. Root must admit preparation
work, complete stack use and caller output independently of this API.

CPU tests compile emitted programs both as a native executable and as a
`no_std` Rust library and compare checked Source byte/refusal outcomes. They
also verify allocation-free resident construction/evaluation and refusal of
invalid code before execution. Static lifetime does not prove physical flash
placement. `StaticScalarExpressionBack` in the semantic catalog wraps this
image using the same expression Back lifecycle as the owned evaluator: staged
output, atomic commit, closing Flow, cancellation and reusable Value sessions.
Its fixed result buffer is 16 bytes. Before evaluation it charges the maximum
instruction count; a successful Step also needs one send and one consume. The
installing Host must admit that complete fuel grant, Back state and evaluation
stack and bind the exact Source/Fore/artifact before offering it. CPU tests run
this Back in the actual fixed kernel with zero allocations and verify atomic
refusal under insufficient fuel. This library Back is not an installed offer.
Ordinary Make selection, full structured protocol images and physical MCU
admission remain unfinished. Native-law/quantity profiles without a retained
validation witness remain unsupported. Checked input/output value contracts
also refuse emission until their validation witnesses can be retained; extracting
the arithmetic tape must not erase the owned evaluator's boundary checks.

## Owned tape profile

Issue #5540 adds an explicitly selected std scalar specialization with Value Fore contracts within the existing checked pure-expression Kind, ordinary Back offer, Plan and kernel Host Call mechanism. The new profile/implementation/artifact identities are distinct from the reference evaluator. Text, records, mathematical-value/quantity/floating profiles, Flow contracts and oversized tapes refuse preparation; the existing interpreter remains independently available. No candidate switches execution form halfway through Play.

The pre-Play lowering owns a maximum 128-instruction linear tape and a fixed 128-slot stack scratch bound, exposed together with the existing output capacity. It drops the evaluator's AST execution form. Runtime steps use prebound operand slots and the existing checked fixed-integer/boolean arithmetic routines. Conditional jumps are forward-only and retain lazy selected-branch semantics. This removes recursive AST traversal, not every primitive decode; it is neither generated machine code nor a second scheduler.

Input/nominal capsule validation and output framing remain the existing evaluator's responsibility. Canonical Source/program provenance remains in the offered contract and Plan. Overflow, invalid input and unsupported profile outcomes remain explicit. Mandatory lowering/storage allocation occurs before Play.

The actual std kernel test selects the specialized implementation/artifact for the checked 1/3/8/32 corpus and compares delivered bytes with the ordinary reference. It retains exact Plan/Play identity and Host Call request/completion signs, unchanged value-store capacities, and zero allocations after Play start. The allocation probe found six legacy Value Fore delivery clones; a responsibility-specific helper now prepares both owned adapter/report buffers and metadata from Plan-sealed byte limits before Play. Each Value output consumes one prepared slot; repetition and oversize refuse. Flow delivery retains its original path and is not offered by this tape profile.

Portable tests cover exact byte/refusal parity, zero warm allocations/reallocations, multiplication/Boolean cases, lazy conditional branches, record/text refusal, a 127-instruction accepted shape and named instruction-bound refusal of a larger valid reference program. Direct delivery tests classify their preparation separately from allocation-free successful delivery.

This is scoped native component and ordinary std Plan/Play evidence. No speedup, arbitrary code generation, JIT, browser/no_std installed tape availability or tiny-Host suitability is claimed. #5541/#5542 and the parent retain their independent resource/evidence/performance/cross-target acceptance.
