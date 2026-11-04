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
validation witness remain unsupported. Fixed-integer boundary contracts can
retain their byte ceiling and range endpoints in immutable
`FixedIntegerContractSpec` data. The resident view is one pointer; identical
input/output guards share one emitted spec. The owned and borrowed contracts
use the same range definition and validation routines. Other boundary profiles
still refuse emission rather than erase checks.

`computation_work()` includes bounded predicate validation as well as tape
instructions, and the shared resident Back charges that amount before evaluation.
Malformed input is rejected before semantic predicates, preserving the canonical
reference's refusal order. A failed output predicate leaves caller output
unchanged. No Source or contract decoder is invoked during resident execution.

## Resident fields from a structured Native input (development)

`generate_static_projected_expression` resolves fixed field payload offsets or
retains checked field/index/variant paths on an authoring Host, beside the same
scalar instructions. Finite sequence lengths and variant tests use those same
borrowed typed paths. The
resident `StaticProjectedExpression` first checks the exact Native input with
its retained schema and fixed predicates, then supplies the selected leaf bytes
to the existing scalar loop. It owns no field evaluator, Type tree or result
buffer. Output predicates run before copying into caller storage. Program
correlation does not grant authority; installing this profile still requires
exact Source/Fore/artifact binding and full admission of validation work,
traversal, state, stack and output.

The emitted real HT16K33 conditions and independent multi-field arithmetic cases
compile as native and no_std modules and preserve ordinary admission plus
reference outcomes without allocation. Missing checked owners, unsupported
Native invariants, inadequate input capacity, constructed selection sources and
Native intermediate constants without law witnesses refuse emission. Common
Back installation and physical execution remain unfinished.

## Resident structured constructors (development)

`generate_static_structured_expression` emits the existing constructor tape as
immutable data, using the same compiler and loop as the owned profile. Fixed
input-node copies preserve canonical framing. Scalar child instructions share
one input Native table. A supplied scratch buffer holds the complete result;
the output Native guard checks it before atomic copying into caller output.
The resident evaluator owns no instructions, constants, Type tree or output
buffer. Its static descriptor declares finite input/output ceilings, and its
constructor rejects invalid indices, slices, backward control and oversized
code. Complete Root workspace and execution-work admission is still required.

All five HT16K33 expressions now have emitted native/no_std component proof,
including variable payloads, every bus-result tag, pressure and refusal/reuse.
Selected canonical nodes can be copied directly from an already checked input.
Make now prepares and binds resident code to a selected sealed Plan; ordinary initialized-provider/Seed installation and physical Play remain unfinished. Source's refined-construction admission rule is preserved; the
output-predicate test is an explicit portable-program component fixture.

## Structured resident Back lifecycle (development)

`StaticStructuredExpressionBack` wraps the resident image in the existing
`PreparedExpressionBack` lifecycle. It owns fixed result/staging arrays chosen
by Make, checks capacities and rejects work that cannot fit the kernel's u16
Step grant. The installing Host must bind exact Source/Fore/artifact and compare
required fuel with the Plan-sealed grant before publishing an initialized Back.
No offer, authority or reservation is created by the library constructor.

Cooperative work counts bounded canonical-validation passes, path/selection
calls, fixed predicate checks, scalar instructions and constructor/publication
operations. Canonical byte/depth ceilings and stack/workspace must be admitted
independently; these units are neither CPU instruction counts nor hard timing or
preemption guarantees. Iteration through collection paths uses a conservative
node bound derived from the admitted canonical byte envelope. Both branches and
all child operations contribute to the bound. The common lifecycle preflights
private work plus send/consume fuel before evaluation or any IO intent.

The parent validates the whole input once. A private child entrance reuses that
view only after constructor-time equality of exact schemas, predicate/scope
tables, canonical capacity and occurrence limits. Standalone child calls retain
full checks. No public admission witness or authority is inferred from the
borrowed view. Structured evaluation refusals remain inspectable on the Back.
Actual frame-request generated-code tests cover bounded kernel delivery,
pressure, insufficient fuel, uncommitted retry, committed reuse, closing Flow,
terminal cancellation and invalid/mismatched capacity/work/guards without
allocation. Make preparation and selected-Plan binding are available on the development branch. Initialized offers, Seed installation and physical execution remain unfinished.

## Owned tape profile

Issue #5540 adds an explicitly selected std scalar specialization with Value Fore contracts within the existing checked pure-expression Kind, ordinary Back offer, Plan and kernel Host Call mechanism. The new profile/implementation/artifact identities are distinct from the reference evaluator. Text, records, mathematical-value/quantity/floating profiles, Flow contracts and oversized tapes refuse preparation; the existing interpreter remains independently available. No candidate switches execution form halfway through Play.

The pre-Play lowering owns a maximum 128-instruction linear tape and a fixed 128-slot stack scratch bound, exposed together with the existing output capacity. It drops the evaluator's AST execution form. Runtime steps use prebound operand slots and the existing checked fixed-integer/boolean arithmetic routines. Conditional jumps are forward-only and retain lazy selected-branch semantics. This removes recursive AST traversal, not every primitive decode; it is neither generated machine code nor a second scheduler.

Input/nominal capsule validation and output framing remain the existing evaluator's responsibility. Canonical Source/program provenance remains in the offered contract and Plan. Overflow, invalid input and unsupported profile outcomes remain explicit. Mandatory lowering/storage allocation occurs before Play.

The actual std kernel test selects the specialized implementation/artifact for the checked 1/3/8/32 corpus and compares delivered bytes with the ordinary reference. It retains exact Plan/Play identity and Host Call request/completion signs, unchanged value-store capacities, and zero allocations after Play start. The allocation probe found six legacy Value Fore delivery clones; a responsibility-specific helper now prepares both owned adapter/report buffers and metadata from Plan-sealed byte limits before Play. Each Value output consumes one prepared slot; repetition and oversize refuse. Flow delivery retains its original path and is not offered by this tape profile.

Portable tests cover exact byte/refusal parity, zero warm allocations/reallocations, multiplication/Boolean cases, lazy conditional branches, record/text refusal, a 127-instruction accepted shape and named instruction-bound refusal of a larger valid reference program. Direct delivery tests classify their preparation separately from allocation-free successful delivery.

This is scoped native component and ordinary std Plan/Play evidence. No speedup, arbitrary code generation, JIT, browser/no_std installed tape availability or tiny-Host suitability is claimed. #5541/#5542 and the parent retain their independent resource/evidence/performance/cross-target acceptance.


## Make binding and compact resident configuration (development)

The embedded Make crate now prepares the structured realization from checked
Native owners, fingerprints its base generated source, and binds generated
Back factories to the exact sealed selection and lowered work grant. The
existing assigned-Plan ABI carries a 55-byte resident configuration digest
instead of repeating canonical program text. It requires the independently
installed exact computation inventory and retains logical artifact, encoded
assignment, runtime-state and execution bounds. See the
[lowering contract](../architecture/plan-kernel-lowering.md#resident-computation-assignment-development).

The actual HT16K33 frame-request program uses 21,526 canonical text bytes; its
sealed component fixture produces a 959-byte assignment under the unchanged
tiny encoded/runtime envelope. This is metadata/component proof, not a whole
Host resource measurement. Generated native/no_std factories enforce the exact
selected fuel without allocation. The fixture is not an initialized Host offer
or a complete ordinary planner/Seed/physical journey.
