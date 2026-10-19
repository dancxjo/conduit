# Shared Fore capture evidence

The adjacent [audit](../../shared-fore-delivery-20261019.md) defines the measured
scope, limits and conclusions. Timings are observations, not a throughput gate.

- `public-raw.tar.xz` retains both supported command captures, including their
  optimized conformance output and exact source-custody receipts.
- `pair-{1,2,3}-raw.tar.xz` retain six fresh CPU-4 worker processes in alternating
  order. Each contains the full original matrix, not only selected fast cases.
- `archive-xz-manifest.json` hashes every archive and every original member byte.
- `allocation-tests.tar.xz` and its manifest retain the measured-source debug
  tests, workspace lint and three separate allocation-test process logs.
- `before-worker.json`, `after-worker.json` and `workers.json` record frozen
  executable digests; executables themselves are not committed.
- `source-bridge.json` compares every implementation-owned file after refresh.
- `three-pair-summary.json`, the per-pair comparisons and the public comparison
  retain slow samples as well as fast samples. They distinguish whole-call and
  acknowledgement-window measurements.

Extract an archive without altering its member bytes. The independent audit
verifier can run on relocated directories; it checks the output path recorded
by the original command when reconstructing its fresh Boot identity:

```sh
tar -xf pair-1-raw.tar.xz
python3 verify.py shared-fore-pair-1-before shared-fore-pair-1-after
python3 verifier-tests.py shared-fore-pair-1-before shared-fore-pair-1-after
```

`verify.py` checks complete capture digests before comparing nine ordinary
owned-adapter cases. It checks exact Source/output/kernel-event/work/capacity
fields and terminal observation correlations within each run. It does not
pretend that different Boots have identical Plan or active-Play identities.
The negative tests independently reject mutations to 12 semantic fields after
deliberately bypassing envelope hashes, and separately reject a raw checksum
mismatch. No capture files are changed by those tests.

Measurements enter through `cargo xtask prove runtime-performance` on the exact
committed source. `capture-pairs.py` records the internal frozen-worker replay
procedure used after that entrance. Its absolute local paths are provenance,
not an additional public product interface.
