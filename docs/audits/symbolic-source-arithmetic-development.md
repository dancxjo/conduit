# Exact symbolic Source arithmetic (private development)

Under #5392, the existing checked `MathematicalValue` arithmetic now retains
supported symbolic operands through ordinary `+`, `-`, `*` and `/`. It reuses the
existing canonical mathematical graph, borrowed validator, prepared builder and
its checked normalization. There is no second expression language or arithmetic
engine. Native fixed integers, fixed-point and explicit IEEE semantics are not
redirected through this path.

Two rational operands keep the previous direct exact arithmetic path, encoding
and refusal behavior. A symbolic operand uses prepared canonical construction.
Both operand graphs are imported with checked local-reference remapping; the
result is mathematical data, not a certificate of intermediate-rounded float
execution. Division requires an established nonzero rational denominator under
the existing builder's domain-proof subset. Unsupported symbolic denominators
refuse rather than receiving a heuristic reciprocal or false numerical bound.

The builder retains its 32-node/depth-16 profile and 131072-byte payload ceiling.
A binary operation has one existing 60-million limb-work meter covering both
input decodes, exact arithmetic or graph import/construction. The maximum
encoding is 33059 bytes. Prepared storage bounds for the existing exact Quantity
profile include the builder when such a node is present; unrelated unsupported
shapes still cannot acquire that profile. This does not establish a complete new
Host memory, timing or precision-planning law.

New graph-construction failures retain separate pure-expression details: division by zero 37,
unsupported domain proof 38, work limit 39, node/depth/extent/output limit 40,
and other mathematical encoding/construction failure 41. Existing causes,
including rational arithmetic detail 4 and root cases 33–36, remain unchanged.
No result is emitted on refusal. Partial private construction is reset before
the next evaluation. Conditional branches remain lazy: preparing a branch does
not execute its binary arithmetic merely because both operands are literals.

## Local proof and limits

Focused Source tests compare the ordinary reference and prepared evaluators
against independently constructed Core graphs. They cover addition, subtraction,
multiplication and rational-denominator division of Pi-bearing and root values;
changing rational and symbolic inputs; exact rejection of symbolic denominator
and zero division; depth-limit refusal and later recovery. The main arithmetic
loops contain 224 guarded evaluations with zero allocations/reallocations.
Prepared `. + 1` construction was observed to make 17 allocation calls, zero
reallocations and peak at 166739 bytes of live allocation payload on the local
Linux build. This includes preparation temporaries, excludes allocator overhead
and stack, and is not a retained-memory or throughput benchmark. The additional
prepared symbolic capacity has a real cold cost; no speedup is claimed.

The first expected-graph fixture incorrectly treated `math/radians(1°)`'s
reviewed `(1/180) × pi` graph as structurally identical to `pi / 180`. Its failure
log is retained. The corrected independent construction matches the actual
Angle conversion law. No universal symbolic equality rule was added to make
that comparison pass.

An authored CLI fixture carries sqrt(2) through a separate exact shift Plot
adding `math/radians(1°)` and a separate scaling Plot multiplying by 3 and
dividing by 2, then feeds the selected `math/enclose` Back. Inspection compares
the complete retained exact source and requested width before `face/text`.
The actual CLI replay passes three tests and seven invocations: three successful
symbolic sources, their three zero-width refusals, and an unsupported-denominator
refusal. The composed Plan contains exactly three mathematical-input/output
expression stages, each selecting the ordinary installed Std expression Back
with its original authored Source span. Zero-width refusal is `InvalidInput`
detail 4; the expression Host Call refusal is `HostCallFailed` detail 38. Neither
emits the success output. The local capture retains exact Source/Plan identities,
logs and hash/roundtrip-verified executables.

This is private, unadmitted implementation. It does not establish symbolic
Quantity transport, physical Unit/family/role custody, two independent physical
and numerical projections, qualified floating-point composites, installed browser
execution, firmware, or the whole original parent acceptance corpus.


The first composed CLI execution correctly exhausted the former interval
profile's 100-million numerical grant. Direct measurement found 108915008 work
units for this nine-node graph. The private interval profile now advertises and
binds a finite 200-million numerical ceiling (840 million including existing
validation allowances); the smaller-grant refusal remains tested. The first
symbolic-denominator CLI assertion also expected `InvalidInput` incorrectly:
the installed pure expression crosses a Host Call and retains `HostCallFailed`
detail 38. Both diagnostic logs are preserved.

## Independently checked delivered packet

The expanded Std enclosure suite passes six tests (the retained cost harness
is intentionally ignored). Its new composed specimen crosses checked exact
transport, a sealed Plan, the actual installed interval Back and kernel delivery.
An independently prepared receiver pins the delivered source bytes, recomputes
the interval under the original producer requirements, and admits the requested
width. An exact zero-width consumer refuses the same nonzero interval.

The producer reports zero post-Play-start allocations and unchanged Value
capacity. Receiver success and exact-consumer refusal also allocate nothing.
A focused replay succeeds under the existing 800-million receiver ceiling;
charged work is 750622272 units, including the conservative 640-million generic
validation allowance. This is metered admission accounting, not a CPU-time or
WCET measurement, producer execution attestation, or physical/source authority.
