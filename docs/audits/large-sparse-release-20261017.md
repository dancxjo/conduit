# Optimized 10,000-node sparse dispatch replay

Development evidence for #5231, measured from clean source e96bc9bac9bb93bc95fd2bb50d5f23f5c8844b5c.

The existing production FixedScheduler fixture admits 10,000 nodes and independent ingress Cords, drives all nodes to Await, then delivers two exact inputs to nodes 63 and 9,999. Full-queue retries refuse without changing sequence custody. Four logical Steps consume the values fairly and return to Idle. The same fixture runs with packed selection and its diagnostic linear candidate enumerator; both share current lifecycle, queue, Host and Sign implementations.

The capture tool now builds each selector once in the optimized profile and invokes that executable in three independent processes. Package rebuild provenance and distinct executable hashes are required. Every process retains raw preparation/advance observations and /usr/bin/time CPU/RSS output. Exact replay must agree across all six processes, and all non-timing work observations must agree within a selector. Python -O is refused because it disables evidence checks; optimized captures with fewer than three processes refuse before creating output.

| Selector | Preparation raw ns | Four Steps plus Idle raw ns | Advance ns per useful output |
|---|---|---|---|
| packed | [4389695, 5241005, 5023143] | [4352, 5211, 5604] | [2176.0, 2605.5, 2802.0] |
| linear | [6790976, 6557257, 13253063] | [107592, 99230, 100417] | [53796.0, 49615.0, 50208.5] |

Packed selection visits four candidates and 480 readiness/summary bytes; linear selection visits 30,000 candidates. Both instrumented scheduler objects occupy 5,623,464 bytes and allocate/reallocate zero times in the ingress-and-advance sample. Complete Sign/remote/value/terminal replay agrees in all six runs.


Whole-process resource observations (seconds and KiB):

| Process | User CPU seconds | System CPU seconds | Peak RSS KiB |
|---|---:|---:|---:|
| packed 1 | 0.00 | 0.00 | 24452 |
| packed 2 | 0.00 | 0.00 | 24276 |
| packed 3 | 0.00 | 0.00 | 24556 |
| linear 1 | 0.00 | 0.01 | 24256 |
| linear 2 | 0.00 | 0.01 | 24444 |
| linear 3 | 0.00 | 0.01 | 24084 |

CPU samples are quantized to hundredths of a second; a reported 0.00 is below that resolution, not zero CPU work.

The per-useful-output denominator is two consumed values, not four logical Steps. Advance spans the four asserted Steps plus the final Idle check; startup, ingress, thread creation and report serialization are outside that interval. Preparation is separately timed. Resource logs cover the whole test process, including preparation and startup; they cannot establish isolated Step CPU or memory. The object size includes test-only counters. The fixture uses a 64 MiB diagnostic thread stack.

This is a same-source selector comparison on a shared host, not a historical old-kernel comparison or a calibrated regression threshold. It proves repeated optimized production-kernel execution and exact replay at the requested sparse scale. The fixture contains no numerical computation, pending Host Calls or quiescence edges. It cannot replace the existing small/dense captures or establish FARGAN, browser, ConduitOS, physical timing or full parent acceptance.

One remaining source-visible traversal is terminal detection in scheduler/execution_region.rs: it checks the active completed prefix and, if all completed, the Cord prefix. This fixture has an early incomplete node, so does not measure that worst case. No CPU-share attribution is made from source inspection. The one-level readiness summary itself still has O(N/64) empty-span probes; its finite bound is not a universal O(1) claim.

Reproduce using the repository evidence tool from a clean committed checkout:

```sh
python3 proof/performance/large-sparse-replay.py --release --runs 3 --output /tmp/large-sparse-release
```

Retained report, complete raw logs and checksum manifest: evidence/large-sparse-release-20261017.json and evidence/large-sparse-release-20261017-raw.tar.gz. The original debug report remains unchanged. No runtime code or scheduling policy changes in this slice.

Validation: all six optimized fixture invocations passed; the linear-reference release unit executable also passed 146 tests (two manual captures ignored). All 25 capture artifact hashes were verified before and after archival. Invalid release sample count and Python -O refusal checks passed. Diff whitespace checks passed. Candidate CI is a separate PR gate.
