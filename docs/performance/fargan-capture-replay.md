# Independent FARGAN capture replay

The retained FARGAN worker compares 24-frame trajectories under fresh
and prepared compact-local, matrix-local or matrix-source-cache execution. Replay checks exact Plan identities,
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
cargo xtask prove runtime-performance --verify-fargan-capture target/matrix-local-capture --source FULL_COMMIT_SHA --worker /retained/fargan_performance
cargo xtask prove runtime-performance --check-fargan-verifier target/matrix-local-capture --source FULL_COMMIT_SHA --worker /retained/fargan_performance
```

Both commands accept `--profile matrix-local|compact-local|source-cache`
(default `matrix-local`) and `--output PATH`, created exclusively. Match the
profile to the captured worker owners; an unrelated profile refuses. The second first
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

The three capture/public validator pairs were recovered from `perf/audit-issue-handoff` at immutable
commit `76069eba0f0376ddb619b375805ff0cf10447376`, under
`docs/audits/evidence/performance-handoff-20261019/verifiers/`.
The repetition bound repairs the reproduced matrix-local empty-capture refusal
gap and applies the same producer bounds to all three profiles. Only canonical
source-bound commands are supported; historical frozen-repeat controllers are
not imported. Historical captures,
private parameters and older timing claims are not imported. The separate
matrix-projection verifier, repeat controllers and other handoff work remain
outside this slice.
