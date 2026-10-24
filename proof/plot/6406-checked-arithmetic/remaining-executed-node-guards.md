# Executed-node law guard implementation boundary

This is a design, not implemented or proven by the arithmetic-only slice.

## One sealed registry, prepared once

`VerifiedLawExpression` should own/share one immutable checked Native table derived
from the opaque complete witness closure and independent family anchors. Recover
and bound metadata once during envelope verification, not once per node. Index
scopes/contracts into that table using the owned/borrowed common admission engine.
Deduplicate by exact Type/schema and compare complete contracts/invariants for
conflicts. Do not construct a CheckedSyntaxDocument or NativeLawWitness for an
anonymous structural root. Its private factory must require the verified envelope
and exact node Type, recomputing the anonymous-record/tuple or Core optional
constructor identity (not accepting a schema-name prefix).

Keep Source closure256 documents/depth16/4MiB and checked metadata4096 entries/4MiB.
Before preparing any guard storage, traverse the bounded entire program and make
an aggregate checked profile: total guard templates/scopes/contracts/invariants,
canonical scratch/input buffers, evaluator buffers, and worst-case executed-node
occurrence work. Require explicit granted maxima and checked addition/multiplication;
per-node4096 caps alone permit an unacceptable multiplicative aggregate. No implicit
budget increases. The first implementation can conservatively refuse an excessive
profile; expose counts, not an invented total-heap-byte guarantee.

## Exact frame packaging, no alternate evaluator

Primitive `evaluate_node` returns `PrimitiveValue`, often representation bytes.
A guard must preallocate an exact Type prefix and canonical scratch and use the
existing nominal framing operation (`nominal::append_output`) for that actual Type.
It must validate the original canonical frame/law context, not cast a bare scalar
to another Type. Place optional guard state alongside PreparedNode.operation;
validate after the selected operation produces its value, before its parent can
consume it. Split disjoint borrows of operation/guard; no self references or unsafe.

Structured evaluator results already carry their exact canonical Type prefix.
Validate them after construction/selection and before parent ChildEvaluator exposes
bytes. Record/Variant Native owners require guards just as Nominal owners do;
Core optional requires exact constructor correspondence and recursive payload
coverage. An anonymous root may contain Native descendants and must run their full
canonical occurrence traversal using the private structural-root factory.

## Concrete seams and optimization hazards

- `expression_prepared/evaluate.rs::evaluate_node`: scalar/nominal internal value
  boundary. Preserve selected Conditional execution; do not check both branches.
- `expression_prepared.rs::evaluate`: actual root result and complete input
  admission before body, including absent/present alternatives and primitive laws.
- `structured.rs::child`/ChildEvaluator: dynamic child evaluation already preserves
  cloned PreparedInput. Constant child fast paths also need a runtime guard or a
  conservative dynamic fallback; preparation must not eagerly execute a dead
  branch's Native law admission.
- `structured_tape.rs`: flattened nested constructors can bypass ordinary child
  boundaries. Initially refuse tape eligibility where any flattened node needs a
  guard; use the same existing structured interpreter, or add explicit guarded
  opcodes with admitted buffers. Never silently drop guards for optimization.
- `member_selection.rs`: dynamic source constructors must be guarded before field
  projection; a path into an already fully admitted input can reuse that input
  admission. Exact output Type/prefix checks remain.
- `sequence_selection.rs`: source and index evaluators retain their exact existing
  order. Whole typed collection laws still apply before exposing one element.
- `equality.rs`: validate complete operands before comparison; earlier unequal
  siblings/lengths/tags cannot hide invalid Native leaves.
- `native_u64.rs`/scalar tape: eligibility must refuse guarded Native profiles
  unless actual selected-node guard coverage is retained, never unwrap nominal
  layers to recover an optimization.

## State/refusals and proof

No result may be committed/emitted before all required actual guards succeed.
Guard failures/cancellation retain sticky evidence; do not call successful-only
retire/reset on failed admission to make reuse appear lawful. The Std owner must
choose explicit terminal refusal policy or a separately admitted reset/evidence
handoff; no new preparation/allocation during Play. Raw inspection bytes are not
an admitted Native output witness. New verified API should generation-gate its
admitted result; existing raw evaluator APIs remain arithmetic-only.

Required cases: input/output and intermediate invalid NativeRecord/Variant;
invalid temporary then scalar projection; unequal earlier equality sibling;
selected invalid vs dead-branch invalid constructor; true/false optionals and
nested collections; constant and tape eligibility; missing/foreign registry owner;
aggregate guard/scratch/work pressure; cancellation/failure evidence; allocator
observed successful reuse and terminal refusal. Source/program/proven bits and
legacy arithmetic semantics remain separately checked. Envelope, compiler Kind
routing and selected Std Back are still owed.
