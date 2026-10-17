# Prepared expression specialization acceptance

Issue [#5396](https://github.com/dancxjo/conduit/issues/5396) delivers one
explicit fixed-integer/Boolean Value profile. Its linear instruction tape
removes recursive authored-tree dispatch. The arithmetic is reviewed compiled
Rust inside a Back, not generated native code for arbitrary source. Primitive
decoding, input validation, output framing, Host Calls and kernel evidence
remain part of execution.

The reference interpreter remains an independently offered realization.
Unsupported text, structured, mathematical, quantity, floating and Flow
profiles refuse tape preparation; a caller may select the reference before
Play. There is no runtime fallback or automatic preference based on these
measurements.

## Sealed preparation and identity

The selected tape offer now carries a typed realization property with maximum
executed instructions, scratch payload, retained instruction/literal capacity
and output-buffer capacity. Ordinary Plan fingerprinting commits to the
property alongside the exact program configuration and implementation,
artifact and execution profile. Installation recomputes the bounds and refuses
missing or different properties before Play. Kernel fuel, Sign reservations,
Host request limits and Value storage remain the existing independent sealed
budgets. Payload bounds exclude allocator metadata, the evaluator object and
whole-Host stack frames; they are not a total process-memory claim.

Lowering admits at most 128 instructions. All jumps go forward, so executed
work cannot exceed that count. Preparation selects a 4/8/32/128-slot fixed
scratch array; literals and instruction storage allocate before Play. The
output buffer is prepared before execution and refuses insufficient capacity.
No dynamic compiler, executable-memory authority or new scheduler is involved.

Source, checked and expanded identities remain recoverable independently of
the chosen implementation. The immutable program remains in configuration;
source diagnostics and logical Gear attribution continue through their
existing paths. Reference and tape Plans and Plays intentionally have different
realization/session identities.

## Acceptance evidence map

| Requirement | Proof surface |
| --- | --- |
| Checked 1/3/8/32-operation source, integer boundaries and overflow | Plot `scalar_tape`; installed std `scalar_tape_tests` and `scalar_tape_parity_tests` |
| Constants, checked multiplication, Boolean operations, lazy conditional and instruction ceiling | Plot `scalar_tape`, including repeated success/refusal transitions and allocation counting |
| Malformed configuration, input extent and foreign provenance | Installed `scalar_tape_parity_tests` |
| Exact source/checked/expanded identity; actual selected Back, scheduler and typed output | Installed std Plan/Play tests |
| Cancellation, terminal/Sign parity, output refusal without retry | Installed parity and `scalar_tape_control_tests` |
| Warm allocations and fixed output/Value capacities | Plot allocation probe and actual std kernel post-Play allocation observations |
| Recoverable prepared bounds and tamper refusal | Installed typed-property inspection and omitted/foreign-property refusal |
| Cold/warm cost and optimized Rust comparison | [Performance report](scalar-tape-performance.md) and [stack-profile comparison](scalar-tape-scratch-20261017.md), with raw receipts |

The timing diagnostic additionally records calling-thread CPU nanoseconds on
Linux for authoring/planning, evaluator preparation, warm batches, optimized
Rust and real installed preparation/Play. Other platforms report absent CPU
measurements. Clock reads add diagnostic overhead; very small boundaries are
not calibrated latency claims. Wall and CPU measurements remain distinct.

## Target boundary and performance interpretation

Std offers `std/kernel-pure-scalar-tape@1`. Browser currently installs its
reference expression evaluator; it does not advertise the std tape profile.
Portable WASM compilation establishes an API availability boundary, not browser
execution or installed tape parity. No no_std or tiny-device tape installation
or physical performance is claimed. This is the named unsupported additional
target boundary allowed by the parent acceptance contract.

The retained measurements show evaluator improvements for useful larger
expressions, cold preparation costs, and a one-operation regression before the
small stack profiles. The later stack-profile capture improves tiny evaluator
cost while its actual single-output std Play does not show an end-to-end
speedup. The optimized Rust reference remains much faster and performs less
work. These observations establish neither universal speedup nor native-speed
equivalence. #5395 retains the wider performance program.

Validation and exact publication acceptance must accompany closure; this
document alone is not a release receipt. Earlier raw measurements retain
their original source identities.
