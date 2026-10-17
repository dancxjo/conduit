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
