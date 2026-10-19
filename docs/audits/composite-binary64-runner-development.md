# Reusable binary64 algorithm execution

Private preparation for the remaining composite-fidelity requirements of
#5392/N5 reuses the existing finite binary64 algorithm engine. Arithmetic,
rounding, signed-zero handling, exact semantic values and final error checking
have one implementation. The existing preparation entrance keeps its behavior.

`PreparedBinary64AlgorithmRunner` admits a fixed template and an explicit map
from inputs to exact-rational Constant or ProjectedConstant steps. Literal steps outside that map,
operation order, rounding mode, tolerance and program bounds stay fixed.
Explicit IEEE constants cannot become mathematical input slots. Source and
physical Type binding of those slots remains a caller admission obligation.

A projected input retains its exact rational source separately from its actual
finite binary64 representation. Its admitted incoming error ceiling is verified
and stays fixed in the template. The runner cannot flatten this input to its
source, treat its bits as the mathematical source, or reproject the original
source in a new rounding mode. Exact and projected slots cannot silently coerce.
Total error includes this incoming difference and every subsequent operation.
This numerical distance evidence does not attest producer execution or replace
Source/physical custody, measurement uncertainty or statistical evidence.

The generic numerical-property codec appends opcode 7 for this distinction;
decoding independently recomputes incoming and final claims. Forged input
ceilings and source fields refuse. All seven legacy opcodes in four rounding
modes retain their captured 144-byte properties exactly. The boxed new variant
preserves the existing operation layout; its heap payload is separately counted
using checked size arithmetic and tested at the admission boundary.

Preparation allocates two complete certificate buffers under an additional
conservative retained-payload bound. Execution updates scratch state and commits
by swapping buffers only after all arithmetic and final fidelity checks pass.
Work spent on refused attempts is charged. An oversized caller budget cannot
increase the template's admitted numerical-work ceiling. Failure returns an
error; retaining the prior certificate is not permission to emit an old value
for the failed input.

Six runner tests exercise changing inputs in all four rounding modes, exact
parity with freshly prepared complete certificates, nonprefix input mappings,
sealed literals, insufficient memory, duplicate/nonconstant/invalid slots,
finite work, total-error refusal and rounded-zero division refusal. Both early
and partially evaluated failures preserve the prior certificate and permit a
subsequent valid execution.

There are 131 guarded exact-slot and five projected-slot runner executions with zero observed allocations or
reallocations across the tested success/refusal/recovery cases. Freshly preparing
the five-step comparator takes four allocations. This comparison is component
allocation evidence: the runner pays for two prepared buffers and does not
claim a cold-start, throughput, WCET, whole-process memory or target speedup.

The cancellation oracle uses `(2^53+1) - 2^53`: the rounded algorithm yields
zero while its exact source is one, with absolute error one. Zero tolerance
refuses; tolerance one admits the actual computed result. A final-only
projection of the exact source yields one and is a different algorithm.

Local validation also passes the existing six algorithm tests, eight point
projection tests, 106 Core library tests, three enclosure tests and three
numerical-realization tests, four float-realization tests, six projected-input
tests and the four-mode legacy property test. Strict Core all-target Clippy, Core without default
features and workspace formatting pass. No full workspace test pass is claimed.

This is a Core component, not an installed composite numerical profile.
Source-to-program/input-slot binding, exact physical unit/family/role custody,
qualified runtime certificate encoding and independent consuming verification, retained
Plan/Play/Sign evidence and qualified accumulated-error refusal remain required.
The existing point-projection factory's multi-operation refusal stays intact.
No PR is opened while the Fore contract PR is held; full exact-source CI and
publication remain necessary before any stable acceptance claim.
