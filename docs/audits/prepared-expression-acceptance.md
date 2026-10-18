# Prepared expression specialization acceptance

Issue [#5396](https://github.com/dancxjo/conduit/issues/5396) delivers one
explicit fixed-integer/Boolean Value profile. Its linear instruction tape
removes recursive authored-tree dispatch. The arithmetic is reviewed compiled
Rust inside a Back, not generated native code for arbitrary source. Primitive
decoding, input validation, output framing, Host Calls and kernel evidence
remain part of execution.

The reference interpreter remains an independently offered realization.
The tape lowers fixed-integer/Boolean computations. Nominal Type descendants
require a retained law witness even inside otherwise unused structured inputs.
Other computational profiles remain unsupported; a constant result may consume
a validated intrinsic record input without lowering record operations. A caller
may select an eligible reference realization before Play. There is no runtime fallback or automatic preference based on these
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
| Native range/where laws without retained validation witnesses | Named preparation and std offer refusal; `scalar_tape_native_laws` and installed offer regression |
| Constants, checked multiplication, Boolean operations, lazy conditional and instruction ceiling | Plot `scalar_tape`, including repeated success/refusal transitions and allocation counting |
| Malformed configuration, input extent and foreign provenance | Installed `scalar_tape_parity_tests` |
| Exact source/checked/expanded identity; actual selected Back, scheduler and typed output | Installed std Plan/Play tests |
| Cancellation, terminal/Sign parity, output refusal without retry | Installed parity and `scalar_tape_control_tests` |
| Warm allocations and fixed output/Value capacities | Plot allocation probe and actual std kernel post-Play allocation observations |
| Recoverable prepared bounds and tamper refusal | Installed typed-property inspection and omitted/foreign-property refusal |
| Executable additional target/profile parity | `scalar_tape_wasm` cdylib and `proof/expression/scalar-tape-wasm.mjs`, run by `cargo xtask check expression-baseline` |
| Cold/warm cost and optimized Rust comparison | [Performance report](scalar-tape-performance.md) and [stack-profile comparison](scalar-tape-scratch-20261017.md), with raw receipts |

The timing diagnostic additionally records calling-thread CPU nanoseconds on
Linux for authoring/planning, evaluator preparation, warm batches, optimized
Rust and real installed preparation/Play. Other platforms report absent CPU
measurements. Clock reads add diagnostic overhead; very small boundaries are
not calibrated latency claims. Wall and CPU measurements remain distinct.

## Target boundary and performance interpretation

Std offers `std/kernel-pure-scalar-tape@1`. Browser currently installs its
reference expression evaluator; it does not advertise the std tape profile.
The executable portable-WASM proof runs the same checked integer/Boolean
1/3/8/32-operation corpus in a real WebAssembly instance. Its 48 valid, overflow
and malformed-input cases match the reference evaluator and independent integer
scaling/Boolean identity oracles. Sixteen repeated paired evaluations per case
allocate and reallocate zero times. Source, checked and expanded identities match
the 128 retained std Plan/Play samples. This establishes target-level evaluator
parity; it does not install a browser tape Back, run a browser UI, or establish
no_std/tiny-device installation or physical performance.

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

## Integer and Boolean operation matrix

Source `929a2aa9a2c18920c9ca02b52312f0e0aa88ba4f` adds the missing Boolean
measurement shapes. The separate checked Boolean fixture has balanced trees
of exactly 1/3/8/32 conjunctions. Both false and true inputs, malformed Boolean
bytes and invalid lengths have untimed reference/tape parity and allocation
checks. Nine Plot tests and twelve installed std tests pass; the installed
matrix also compares terminal/Sign kinds and all three semantic identities
while observing zero post-Play allocations and fixed Value-store capacities.

The clean-source release capture passes 128 result-checked trials and retains
512 raw CPU/wall records in
[`scalar-tape-mixed-cpu-samples.json`](evidence/scalar-tape-mixed-cpu-samples.json).
Every product row names its actual authored Source document, checked/expanded
Plot, Plan, Play, provider and artifact. Paired reference/tape rows retain the
same semantic identities and distinct realization identities. The performance
inputs are integer 7 and Boolean true; false and malformed inputs are separate
untimed conformance cases.

| Domain | Operations | Reference warm CPU, ns/value | Tape warm CPU, ns/value | Reference Play wall, µs | Tape Play wall, µs |
| --- | ---: | ---: | ---: | ---: | ---: |
| Integer | 1 | 71.03 | 51.17 | 10.50 | 9.50 |
| Integer | 3 | 167.59 | 102.10 | 11.11 | 9.98 |
| Integer | 8 | 414.95 | 232.92 | 11.40 | 9.87 |
| Integer | 32 | 1581.61 | 926.27 | 15.03 | 14.01 |
| Boolean | 1 | 60.91 | 39.37 | 11.41 | 10.35 |
| Boolean | 3 | 131.19 | 69.91 | 9.43 | 10.81 |
| Boolean | 8 | 317.95 | 144.11 | 12.08 | 11.10 |
| Boolean | 32 | 1191.39 | 524.57 | 14.02 | 11.11 |

The warmed 32-operation evaluator loops observe about 1.08 million integer
values/sec for tape versus 0.63 million for reference, and 1.91 million Boolean
values/sec versus 0.84 million. These are evaluator-loop throughputs, not
whole-Play sustained rates. The Boolean three-operation actual Play loses in
this capture despite its faster evaluator loop. Boolean evaluator preparation
costs 4.15/4.47/5.33/9.98 µs for tape versus 1.69/1.46/2.06/8.25 µs for reference;
actual installed preparation also costs more for all four Boolean shapes.

The native Boolean comparison reduces an idempotent conjunction tree to its
input, just as the integer comparison uses equivalent checked multiplication.
Its approximately 0.41–0.52 ns/value loop does not perform the product's
primitive validation, dispatch or kernel work. It is a separate optimized
computation reference, not evidence that the tape compiles authored expressions
to native instructions. This capture retains the same uncontrolled-host,
within-process and diagnostic-clock limitations as the preceding capture.

## Native-law eligibility boundary

The scalar tape admits intrinsic fixed-integer and Boolean Types. A nominal
scalar Type commits to its native range/`where` laws in its schema identity,
but `PortableExpressionProgram` does not retain those validation programs.
Both the recursive reference evaluator and the previous tape could unwrap the
representation and accept a forged nominal capsule outside the native range.
Reference parity is therefore insufficient evidence for these profiles.

Tape preparation now refuses nominal input, output, and supported intermediate
node Types with `scalar-tape/native-law-witness-unavailable@1`. The std offer
builder propagates this reason before advertising a specialized Back. Tests
cover range and `where` Types, a law used to prove non-overflow, and an
intermediate nominal Type behind intrinsic ports. No runtime fallback is added.
The ordinary evaluator is not certified as a native-law validator by this work;
future nominal specialization requires retaining and enforcing its validation
witness before erasing any checks. This is the named unsupported-profile branch
of #5396 acceptance, not a claim of nominal support.

The retained CPU captures predate this admission guard. Their checked intrinsic
integer/Boolean sources do not contain nominal Types; their exact source pins
and raw measurements remain unchanged.


## Executable portable-WASM parity

`cargo xtask check expression-baseline` retains the native baseline and then
builds the dedicated proof cdylib for `wasm32-unknown-unknown` and executes it
using Node's WebAssembly engine. This developer check requires that Rust target
and Node. The module has no ambient imports. It invokes the existing Plot
reference and scalar-tape evaluators; there is no additional evaluator or kernel.
The shared allocation probe observes only the prepared evaluation windows.

The receipt retains exact programs, source/checked/expanded identities, inputs,
outputs or named refusals, instruction/scratch/output bounds, and allocation
counts. The runner checks all eight profiles, fixture hashes and semantic
identities against the installed std raw evidence. It also checks that a second
invocation refuses without rerunning or changing the one finite receipt.
This isolated harness is a portable evaluator proof, not a deployable Host Back.

Clean source `1d28dd2d5f2ba78b4f8fa8270f9e7f4535624ca1` passes the complete supported check,
WASM-example and tooling strict Clippy, and formatting. The
[WASM receipt](evidence/scalar-tape-wasm-parity.json) retains all 48 cases,
the actual module hash, compiler/runtime identities and matched std identities.
The [validation record](evidence/scalar-tape-wasm-validation.json) retains
the command and log hashes. This is development proof; exact stable publication
acceptance remains required before parent closure.

## Arithmetic hints are not retained proofs

A forged portable Binary `proven` flag reproduced wrapping for U64 overflow.
Tape lowering now binds checked addition, multiplication and subtraction even
when this bare annotation is true: the flag carries no retained range witness.
The regression checks successful input against the original checked Source and
requires arithmetic refusal for overflow/underflow for all three operators.
Valid proven programs remain eligible; this does not claim the reference
interpreter validates forged annotations or nominal native laws. All 12 Plot
evaluator/eligibility tests and 13 installed std tape tests pass; focused strict
Clippy passes. Existing CPU receipts retain their historical source identities;
the measured fixtures already used this checked instruction path.

The supported `cargo xtask check expression-baseline` suite also passes from
clean source `36c978f82363877cd3beb0dad74e7a787d8696af`: two native baseline
tests and the real WASM parity/refusal/allocation corpus. Its
[checked-binding receipt](evidence/scalar-tape-wasm-checked-hints.json) and
[validation record](evidence/scalar-tape-wasm-checked-hints-validation.json)
retain that source, the runtime hash and the same 128 std semantic identity
matches. Earlier WASM and CPU receipts remain unchanged.

## Combined platform-repair validation

Clean local source `0601beb149f6e3bf18cb5a545fd4a7bdad4acea5` combines this
feature with its own platform-repair branch. It passes all 12 Plot scalar tests,
13 installed std scalar tests, focused strict Clippy and workspace formatting.
The supported native/WASM suite passes its two native baseline tests and all
48 WASM cases. The [combined-tree receipt](evidence/scalar-tape-combined-repair-wasm.json)
and [validation record](evidence/scalar-tape-combined-repair-validation.json)
retain this source and the original log hashes. Earlier source-specific receipts
remain unchanged. This is local interaction validation; repair admission, fresh
feature Candidate, exhaustive dev integration and exact-main publication remain
separate required gates.

## Native laws inside unused structured inputs

A checked constant expression with a record input containing a nominal field
reproduced an eligibility gap: the top-level input record passed the earlier
nominal check even though its field had no retained law witness. Preparation now
walks record fields, variant payloads and collection/sequence elements before
admitting the tape. The same named native-law refusal applies throughout.
The new negative regression fails before this correction; a positive regression
keeps a valid intrinsic record input supported and verifies its constant result.
All 14 Plot scalar tests pass after the correction. Earlier receipts retain their
original source identities and do not constitute proof of this new guard.
