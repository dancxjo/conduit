# Alternating same-source selector observations

At source `af28cf2b2bf2a022028b61d555fd7003cb6d661e`, rebuild both real Cargo feature variants of the existing production FixedScheduler fixture with identical release opt3/overflow/profile settings. Copy and hash both freshly built executables before the next package clean. Run nine process pairs on one allowed CPU, alternating packed→linear and linear→packed order. Each process executes the existing eight trials of loaded/sparse 1/8/32/256-node specimens. This is the retained kernel observation fixture, not a new benchmark Back or scheduler.

All 1,152 matched samples retain exact full Sign, remote, received-output, readiness, value, terminal and object-size parity, with matching zero-allocation/copy and non-selection work observations. The diagnostic legacy candidate enumeration shares current packed storage and all other kernel machinery; it is not a reconstruction of the original Boolean-array implementation.

The table reports mean fixture advance nanoseconds and the range of the nine packed/linear process-pair ratios. Preparation remains a separate raw field. Sparse fixtures receive one useful value; loaded fixtures receive one per node. The interval includes instrumented Steps, assertions and required Signs, rather than isolated selector instructions.

| Nodes | Pattern | Packed mean ns | Linear mean ns | Paired ratio range |
| ---: | --- | ---: | ---: | --- |
| 1 | Loaded | 546.2 | 711.8 | 0.250–1.281 |
| 1 | Sparse | 330.8 | 320.8 | 0.839–1.200 |
| 8 | Loaded | 1242.0 | 1289.4 | 0.848–1.076 |
| 8 | Sparse | 295.0 | 330.2 | 0.541–1.081 |
| 32 | Loaded | 3428.0 | 3266.0 | 0.904–1.557 |
| 32 | Sparse | 320.4 | 433.5 | 0.611–0.849 |
| 256 | Loaded | 24181.7 | 23714.2 | 0.957–1.245 |
| 256 | Sparse | 382.9 | 1290.5 | 0.242–0.499 |

Sparse 32/256-node packed observations are below their corresponding linear observations in all nine pairs. One-node and loaded cases include unfavorable packed samples; their pair ranges cross 1. A first-fixture dense outlier also makes the ratio of aggregate means differ from the mean paired ratio. No sample is removed. Affinity is not an exclusive reservation, and frequency, thermal state, background load and timer sensitivity remain uncontrolled. These observations do not establish a calibrated regression threshold, universal tiny-Host timing equivalence or causal speedup. The policy stays unchanged.

The verifier independently checks every retained hash, fresh build/profile, process order, all raw-log/row joins, exact cross-selector semantic replay, non-selection work and recomputed timing summaries. Four mutations refuse: bad hash, duplicate sample, consistently rehashed timing/raw-log disagreement and consistently rehashed altered Sign replay. The receipt and full archive are `evidence/selector-pairs-20261017.json` and its raw archive; the archive includes original executables, raw logs, capture helper, CPU-selection observation, verifier receipt and refusal helper. After extraction, run:

```sh
python3 proof/performance/verify-selector-pairs.py --capture /tmp/selector-proof/5231-selector-alternating --output /tmp/selector-verified.json
```

The extracted `capture-selector-alternating.py` must remain beside the capture directory so its hash is verified. Whole checked Host/Plan/Play and real FARGAN costs are not measured here. The outstanding FARGAN graph, current Candidate and combined-development obligations still prevent full #5231 closure.
