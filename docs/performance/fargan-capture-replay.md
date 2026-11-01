# Independent numerical capture replay

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
private parameters and older timing claims are not imported. Frozen-repeat controllers and other historical handoff work remain outside
this slice.

## Isolated matrix arithmetic

The projection worker uses deterministic synthetic weights and inputs. Its
replay independently regenerates the PRNG values and ordered F32 operations,
checks every output bit, trial coverage/order, weight digest, source/worker
identity and zero warm allocation requests. It is separate from full
FARGAN, Back/Plan, hardware traffic or accepted timing proof.

```sh
cargo xtask prove runtime-performance --matrix-projection --output target/matrix-projection-capture --iterations 1 --repetitions 3 --case-group all
cargo xtask prove runtime-performance --verify-matrix-projection target/matrix-projection-capture --source FULL_COMMIT_SHA --worker /retained/matrix_projection_performance
cargo xtask prove runtime-performance --check-matrix-projection-verifier target/matrix-projection-capture --source FULL_COMMIT_SHA --worker /retained/matrix_projection_performance
```

These replay commands take no FARGAN `--profile`. They accept an exclusive
`--output PATH`. Iterations must be an integer in 1..100000 and repetitions
in 3..101, matching the producer. Corruption controls update outer artifact
hashes before checking altered output bits, weight digests, allocation
requests, duplicate/missing trials and invalid metadata/bounds, so digest
failures cannot mask the underlying semantic refusal.
