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

## Source-pinned CPU and wall capture

Clean source `fccbecf91351fa37630f6b7b0c8f6a71f8dd5280` passes eight Plot
evaluator tests, eleven installed std tests, focused strict Clippy and
workspace formatting. Plot also compiles for `wasm32-unknown-unknown`; no
browser execution is inferred. The release timing test passes with all
64 result-checked trials and 256 raw timing records retained in
[`scalar-tape-cpu-samples.json`](evidence/scalar-tape-cpu-samples.json).

Eight-batch medians on the unpinned Linux i7-12700 host:

| Operations | Reference warm wall / CPU, ns per value | Tape warm wall / CPU, ns per value | Reference Play wall / CPU, µs | Tape Play wall / CPU, µs |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 74.43 / 74.43 | 51.77 / 51.77 | 14.35 / 14.30 | 13.43 / 13.33 |
| 3 | 179.17 / 179.15 | 106.02 / 106.02 | 13.72 / 13.67 | 12.55 / 12.50 |
| 8 | 427.66 / 427.56 | 243.52 / 243.52 | 14.23 / 14.12 | 14.49 / 14.44 |
| 32 | 1620.89 / 1620.79 | 935.16 / 934.98 | 15.46 / 15.38 | 14.63 / 14.55 |

Tape evaluator preparation costs 4.16/4.37/5.55/10.28 µs wall time,
versus reference 2.08/2.39/2.99/9.33 µs. Actual installed preparation costs
120.87/119.96/151.62/204.36 µs for tape versus
98.07/103.40/116.06/164.38 µs for reference. The new property is recomputed
during preparation and contributes to that cost; this capture does not
isolate its causal cost. CPU observations, raw variance, authoring/Plan
cost and immutable execution identities are retained separately in the receipt.

The warmed tape evaluator improves in all four shapes in this capture.
Actual Play is noisier and the eight-operation case is slightly slower.
Cold preparation remains more expensive. The optimized Rust loop measures
about 0.42–0.43 ns per value and omits the product's surrounding obligations.
No compiler jobs overlapped this capture, but ordinary system activity,
frequency and placement remain uncontrolled. These within-process batches
are descriptive measurements, not independent confidence intervals or a
universal selection rule.
