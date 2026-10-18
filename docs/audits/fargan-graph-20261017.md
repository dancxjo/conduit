# Executable FARGAN frame and scheduler workload

For #5231, the optional speech Source profile now composes the complete
conditioner and four recurrent subframes into one 160-sample frame. The ordinary
Plan uses 768 nodes and 1,211 Cords; generic AI numerical owners retain admitted
immutable tensors. Network order and state transitions remain authored Source.
The [profile and recovery boundary](../../proof/fargan/README.md) identifies the
upstream pin, public preparation API, private parameter boundary and separate
speech-product work.

The [retained baseline receipt](evidence/fargan-graph-20261017.json) and
[raw archive](evidence/fargan-graph-20261017-raw.tar.gz) contain five independent
24-frame trajectories on CPU 1 through the production kernel, with explicit
warm initial state and each trajectory's own accumulated history/state. Every
frame reaches structural drain with no outstanding Host Call completion and
zero Play allocations or reallocations. All repeated exact Plan, output,
Sign and invocation counts match. Fifteen artifact hashes are independently
verified; ten corrupted captures are refused, including re-signed false
allocation, custody, phase, Plan, Sign and output records.

| Observed phase | Mean per frame |
|---|---:|
| Complete Play, including expression Host Calls and structural drain | 21.902 ms |
| Process CPU during Play | 21.902 ms |
| Numeric Backs, including codecs | 4.306 ms |
| Expression Host Calls | 12.847 ms |
| Step remainder: kernel, transport, Signs, Back commit and observers | 4.674 ms |
| Per-invocation owner preparation, separately measured | 2,818.002 ms |

Raw samples also retain first-use Source/model-resource admission, each cold
Plan, per-Step work counts, all 6,976 Signs, exact result hashes and memory.
The fixed scheduler occupies 2,642,384 inline bytes in this explicit hosted
profile. Allocator live/peak totals also include prepared Plans, immutable
model resources, catalogue caches and proof fixtures; they are not a minimum
product heap. The proof worker requests a separate 32 MiB thread stack.
Process-first-use measurements do not imply a cold OS file cache.

The scalar model comparison's maximum absolute PCM error is 8.781e-6;
active signal-state error is 0.013958 and conditioning-history error 5.866e-5.
The explicit libm profile differs from upstream activation approximations.
The independent development C reference is never linked into Play, and its
parameters are not redistributed in this evidence.

Expression Host Calls dominate this Play. The expensive repeated preparation
is also visible: these are freshly prepared Value invocations, not a reusable
stream or a real-time vocoder claim. Interpretation/fusion remain #5396/#5397
work; neither is credited to local ready-set selection. This baseline precedes
the held terminal-membership and Host-blocked wake changes. Their separate
before/after comparison and combined-development acceptance remain outstanding.
