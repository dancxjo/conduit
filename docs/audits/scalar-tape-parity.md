# Scalar tape differential conformance

Development proof for #5541, dependent on #5540. The admitted profile is fixed
integer/Boolean Value input/output, at most 128 lowered instructions. The tests
use checked Source and genuine baseline/tape Std offers to seal separate Plans.
Source document, checked and expanded identities remain equal; Plan, provider,
artifact and Play identities deliberately describe their actual realizations.

The focused tests compare negative and boundary signed values, overflow, invalid
input lengths, invalid canonical storage bounds, foreign artifact provenance and
malformed serialized configuration, sealed byte-contract violations and duplicate
Value input refusal before delivery. Actual installed kernel tests compare output
bytes, normalized terminal dispositions and event kinds, including cancellation
before input. A refusing output adapter receives exactly one delivery attempt;
its error survives without retry or fabricated completion. This adapter test is
an effect-boundary refusal, not an assertion of kernel queue-pressure signaling.

Existing #5540 tests retain the exact checked 1/3/8/32-operation corpus, zero warm
allocations/reallocations, checked multiplication, Boolean negation, lazy
conditional branches, unsupported text/record/Flow refusals, and the 127/129
instruction admission boundary. Valid unsupported programs remain eligible for
the explicit reference offer; the tape never silently falls back during Play.

Runtime bounds are structural: the private tape is produced only by bounded
preparation, jumps target later instructions, each executed instruction advances
the program counter, and evaluation has no recursive call stack or backward
edge. At most 128 instructions execute, using exactly 128 stack slots (5120 bytes
on the measured native ABI); prepared literal/instruction/output storage is
retained before Play. Kernel scheduling fuel and Sign storage remain the existing
sealed-plan budgets. These are distinct bounds, not a claim that expression
instructions become individually scheduled kernel steps.

No browser/no-std/tiny-device execution or physical target claim is made here.
Parent #5396 retains its performance and broader adoption requirements.

Focused native tests: ten passed with zero failures. This is installed native
kernel and deterministic evaluator evidence, not physical-device proof.
