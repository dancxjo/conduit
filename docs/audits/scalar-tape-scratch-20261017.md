# Scalar tape stack profiles selected before Play

Local #5396 implementation slice coordinated with #5395, 17 October 2026.

The fixed-integer/Boolean tape previously initialized a 128-slot evaluation
array for every expression. Preparation now chooses one of four finite stack
profiles: 4, 8, 32 or 128 slots. Each profile runs the same instruction loop,
with a separate fixed-size array. Keeping those loops out of line prevents the
maximum array from becoming part of every tiny evaluation's stack frame.

The 128-instruction admission ceiling, branch/overflow behavior, borrowed input
and literal bytes, output buffer and explicit profile selection are unchanged.
No new heap scratch or persistent evaluated values are retained. Instruction
and literal allocation still happens during preparation. Scratch-limit inspection
reports the chosen array payload bound; it does not establish whole-Host stack
usage, total memory or code footprint. Four compiled loop variants can add code
size, and the prepared tape now retains its finite scratch-profile selector.

On this native ABI, the 1/3/8/32-operation fixtures select 160/320/1280/5120
scratch bytes, respectively, versus 5120 bytes each before. The largest profile
retains the original array ceiling. No installed browser/no_std/tiny-device tape
profile or physical performance claim follows from a portable compile check.

Eight evaluator tests cover all four profiles, identity, the 127/129-instruction
admission boundary, bytes/refusals versus the reference, and repeated transitions
between lazy branches, overflow and success without allocation/reallocation or
capacity growth. Ten installed std tests exercise actual selected tape Plans,
terminal/Sign parity, invalid frames/provenance and adapter refusal.

## Reusable-buffer experiment retained

An initial implementation at `f3069d9db` moved scratch into an exact-size
pre-Play reusable vector and owned inline scalar bytes. It passed correctness
and allocation tests. Its warm one-operation median improved 29%, but the
32-operation median increased 22% while the unchanged reference increased
about 11%. That result did not justify retaining the vector/copy tradeoff.
The final stack profiles preserve the original borrowed-value loop and avoid
adding a heap allocation. The rejected experiment and all its unfavorable
samples remain in `evidence/scalar-tape-reusable-experiment-20261017.json`.

The existing ignored `scalar_tape_performance` std release test supplies the
report-only diagnostic. Each clean-source invocation retains eight rows per
1/3/8/32-operation shape for reference and tape: 100,000-evaluation warm batches
and real sealed-Plan preparation/Play with validated output. These are single
processes with within-process batches on an uncontrolled host, with no affinity
or frequency control, independent confidence interval, stable regression gate
or native-code compilation claim. Compiled test binaries are not retained.

## Final fixed-stack observations

Before source: `d1930a8fab3956e96e61f4ff49e30c73df4ed7bf`. Final source:
`d654cd10011b7128f6a3cf78666683e5afa3a6b8`. The worktree stayed clean
through each measurement; checked fixture and benchmark source
bytes are identical between commits. Final scratch limits also confirm the
new implementation, independently of the operator-supplied source-pin string.

Warm evaluator medians, nanoseconds per value (eight 100,000-value batches):

| Operations | Tape before | Tape after | Reference before | Reference after | Scratch before → after, bytes |
|---:|---:|---:|---:|---:|---:|
| 1 | 90.1 | 49.5 | 70.0 | 70.8 | 5120 → 160 |
| 3 | 141.3 | 102.8 | 166.6 | 166.9 | 5120 → 320 |
| 8 | 269.9 | 235.6 | 413.4 | 415.7 | 5120 → 1280 |
| 32 | 907.8 | 915.7 | 1569.0 | 1578.6 | 5120 → 5120 |

The final tape's warmed medians improve about 45%/27%/13% for 1/3/8
operations, with the 32-operation observation +0.9%. Reference warm medians
are within 1.3% across epochs. These are descriptive observations; this
does not establish a reviewed regression threshold or independent confidence.

Real std single-output call boundaries, tape medians in microseconds:

| Operations | Back preparation before → after | Play before → after |
|---:|---:|---:|
| 1 | 68.50 → 70.23 | 8.64 → 8.98 |
| 3 | 72.20 → 78.86 | 8.55 → 9.09 |
| 8 | 82.36 → 90.98 | 8.68 → 9.23 |
| 32 | 148.06 → 150.65 | 9.73 → 10.82 |

The one-output real Play observations do not show an end-to-end speedup. Host
coordination and evidence still dominate these microsecond-scale calls; all
unfavorable samples remain in the receipt. Standalone tape evaluator preparation
remains about 3.6/3.8/4.9/9.7 µs for the final 1/3/8/32-operation cases.
This explicit Value-profile change does not accelerate the ordinary Flow
reference profile measured by the earlier 32-Gear phase study.

Evidence: `evidence/scalar-tape-scratch-20261017.json` retains all 256 final
comparison rows, statistics, source/fixture/benchmark/log hashes and recorded
Plan/Play/provider/artifact IDs. The rejected-vector receipt and raw log remain
separate. Environment metadata is a post-study snapshot, not a retained per-run
frequency/affinity observation. The original ignored test uses
`CONDUIT_SCALAR_TIMING_SOURCE` and `cargo +stable test -p conduit-std-host --release --test scalar_tape_performance -- --ignored --nocapture`; this is an internal developer diagnostic, not a new public command.

Local focused checks pass: eight debug and eight release evaluator tests, ten installed std tests,
strict plot Clippy, workspace formatting and the portable compile check.
No CI requested. Parent #5396 remains open for wider adoption and acceptance.

Environment maintenance used `cargo +stable clean --profile dev -p conduitos`
to reclaim 2.8 GiB of inactive regenerable debug artifacts. Release artifacts,
source, branches and unique retained evidence were preserved.
