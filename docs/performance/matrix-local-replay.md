# Independent matrix-local capture replay

The retained matrix-local worker compares 24-frame trajectories under fresh
and prepared matrix-local execution. Replay checks exact Plan identities,
framed artifact hashes, final outputs, per-node ordered Value transactions,
finite fusion membership, and zero Play allocation requests. It does not
establish quiet timing, a speedup, ordinary Body integration or release acceptance.

Capture through the existing entrance with an explicitly authorized private
pinned model/oracle directory; no inputs are downloaded:

```sh
cargo xtask prove runtime-performance --fargan-fixture /private/fixture --output target/matrix-local-capture --repetitions 2 --fargan-owners prepared-matrix-local
```

Retain the actual worker executable alongside the capture. Supply the full
expected source commit and that worker when independently verifying:

```sh
cargo xtask prove runtime-performance --verify-matrix-local target/matrix-local-capture --source FULL_COMMIT_SHA --worker /retained/fargan_performance
cargo xtask prove runtime-performance --check-matrix-local-verifier target/matrix-local-capture --source FULL_COMMIT_SHA --worker /retained/fargan_performance
```

Both commands accept `--output PATH`, created exclusively. The second first
verifies the original capture, then checks independently corrupted scratch
copies without modifying it. The capture directory must support hard links.
These Python diagnostic checks refuse optimized Python execution that disables
assertions.

Replay rejects missing/duplicate frame rows, altered digests, nonzero Play
allocation, incorrect schema/frame count and repetitions outside the producer's
integer 2..20 envelope. Every retained artifact and the exact worker hash must
match the source-bound command receipt. The worker's ignored fixture tests are
not counted as passing: explicit conformance logs and actual trajectory
artifacts are checked separately.

The two validators were recovered from `perf/audit-issue-handoff` at immutable
commit `76069eba0f0376ddb619b375805ff0cf10447376`, under
`docs/audits/evidence/performance-handoff-20261019/verifiers/`.
The repetition bound repairs a reproduced refusal gap in the historical
validator, which accepted an empty zero-repetition capture. Historical captures,
private parameters and older timing claims are not imported. The separate
matrix-projection verifier and other handoff work remain outside this slice.
