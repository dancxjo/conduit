# Production std streaming measurement for #5395

The previous finite sequence runner drained kernel work between each input.
The new explicit stream entrance offers a bounded batch while prior values are
still executing, using the same std Back factories, sealed Fore routes, atomic
remote fan-out, kernel queues, fuel, cancellation and Sign storage. Causal
sequence behavior remains unchanged. Its finite profile admits 1–4,096 inputs
and 1–64 admission attempts per kernel turn; queue and byte capacity remain
Plan-owned. Preparation validates every input before Play. Full retries keep
the exact payload and per-Fore sequence. Closure waits for drained work.

Fourteen supported hosted cases measure 1,024 useful input values: matched
1/8/32-Gear 32-addition chains, one/four fan-out branches, queue capacity one/four,
batch one/eight, fast/synchronous-slow acknowledgement and 32/256-byte Text
identity payloads. The existing 256-byte checked Text envelope is preserved;
512 bytes refuses before execution. The slow sink is a real synchronous Host
acknowledgement sleep, not a background scheduler. OS oversleep is included.

The fixed-capacity collector timestamps every acknowledgement, preserves exact
per-branch order and outputs, reports observed gaps between each branch's
completed values, and checks every native control. The fused native and
iterator-library controls preserve canonical data and the same sink dwell.
Their omission of typed Plan/Sign transactions is explicit. Compilation and
conformance are outside measurements; mode order rotates by repetition.

Conformance covers actual ordinary streams, pressure, fan-out, storage stability,
mandatory output Signs, malformed frames, overflow, cancellation, unplanned
Fores and oversized Text. Ingress model tests also prove no admission-path
allocation under bounded pressure and preserve legacy causal sequencing.

Raw rows and the independent verifier retain/check source and exact Plan/Host/
Boot/Play custody, per-output hashes, terminal evidence, queue/batch counts,
latency offsets and percentiles, CPU/allocator/phase observations, native output
parity and canonical boundary byte counts. Internal-copy hardware counters are
unavailable, not zero. The performance gate stays report-only.

Measurements and immutable source-bound receipts follow below after the
supported clean-source capture. This slice does not close the parent issue.
