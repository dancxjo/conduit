# Skip proven-disjoint cord prefixes during preparation

Development slice for #5231's preparation-versus-execution breakdown and #5395's
cold-path attribution. Each cord previously compared its queue interval and
remote endpoint against every earlier cord. The ordered 10,000-cord regression
observed 49,995,000 prefix comparisons before repair and zero afterward.

A temporary preparation helper keeps the maximum prior queue end and maximum
prior remote endpoint ID. A new queue starting at or beyond the maximum end is
certainly disjoint. An absent remote endpoint, or an ID above the previous
maximum, is certainly unique. If both facts hold, no prefix scan is needed.
Otherwise the original pairwise checks remain as the fallback. Bounds update
only after successful validation. The helper is called at the original check
position, preserving validation/refusal order. Queue ends remain widened before
addition, including valid arenas crossing the u16 address boundary.

This adds two scalar scratch bounds during preparation, no heap allocation,
sorted scratch array, per-scheduler field or Play work. It does not require
monotonic input: unordered valid layouts remain accepted, while duplicate
remote IDs and overlapping intervals still refuse. Worst-case unordered
preparation remains quadratic. Other topology/route checks are unchanged and
can have their own costs; this is not a universal linear preparation algorithm.

## Local proof and observations

The prefix oracle exercises 512 three-cord combinations spanning unordered
queues/IDs, touching and overlapping ranges, duplicate IDs, local cords,
ingress/egress duplicates and high slot addresses. Its accept/refuse result
matches the original pairwise law. The real 10,000-node scheduler capture from
`97175e74d` is compared with `7d7b08b57` (full identities are in the receipt).
Both captures use the same driver/compiler/profile and direct admitted ingress
topology. Complete retained Sign/remote/gap/output/storage replay and every
non-timing runtime work counter match under packed and linear selectors.
The instrumented scheduler object extent remains unchanged.

| Selector | Before preparation | After preparation |
|---|---:|---:|
| Packed | 1,894 ms | 7.1 ms |
| Linear reference | 1,916 ms | 8.2 ms |

These are single local native **debug** observations of ordered ingress topology
construction, not repeated optimized performance measurements, a small-graph
regression gate, a release speedup factor, or a full authored Plan/Play journey.
The deterministic pair-work reduction and exact replay are the strong results.
The existing diagnostic 64 MiB thread stack and other proof limits still apply.

Raw post-change capture is in `evidence/cord-prefix-preparation-raw.tar.gz`; the
baseline is retained in `evidence/ten-thousand-sparse-replay-raw.tar.gz`. Extract
both outside the checkout, then reproduce the 14-hash comparison with:

```sh
python3 proof/performance/verify-cord-preparation.py \
  --before /tmp/before/sparse-large-97175e74d \
  --after /tmp/after/cord-prefix-capture \
  --output /tmp/cord-prefix-comparison.json
```

The verifier checks successful test logs against receipt rows, artifact hashes,
compiler/profile/harness agreement, distinct executable/source identities,
exact replay and non-timing runtime parity. Current checked output is
`evidence/cord-prefix-preparation.json`. A fresh capture uses the documented
`large-sparse-replay.py` entrance from a clean committed checkout.

Local validation: both prefix tests and both source-pinned fixed-only selectors
passed; alloc and linear-ready-reference kernel suites each passed 203 tests.
Formatting, strict alloc all-target kernel Clippy, and diff whitespace passed.
Parent acceptance, optimized small/large preparation measurements, browser and
physical proof remain open. No CI was dispatched or awaited.
