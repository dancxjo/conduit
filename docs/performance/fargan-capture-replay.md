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

## Complete source-cache comparison

Compare a matrix-local capture with a matrix-source-cache capture after
independently verifying both commands and their explicit expected identities:

```sh
cargo xtask prove runtime-performance --compare-source-cache /captures/matrix --against /captures/cache --source BEFORE_SHA --worker /retained/before-worker --against-source AFTER_SHA --against-worker /retained/after-worker
cargo xtask prove runtime-performance --check-source-cache-comparison /captures/matrix --against /captures/cache --source BEFORE_SHA --worker /retained/before-worker --against-source AFTER_SHA --against-worker /retained/after-worker
```

Both accept exclusive `--output PATH`. Complete Plan files, all frame receipts,
output/event bytes and store/work counters must match; timing is excluded.
The negative check constructs changes to formatting or otherwise unobserved
request fields which each capture's semantic replay accepts independently,
then proves the stricter complete comparison refuses them.

## Retained compact factory control logs

```sh
cargo xtask prove runtime-performance --verify-compact-factory-controls /retained/serial-controls.log --source CALLER_SOURCE_SHA --worker /retained/caller-worker
cargo xtask prove runtime-performance --check-compact-factory-verifier /retained/serial-controls.log --source CALLER_SOURCE_SHA --worker /retained/caller-worker
```

These commands inspect a retained serial diagnostic transcript, including all
13 schema-2 control rows and successful five-test linear and five-test
projection suites from the matrix-projection-reference profile. They check
exact outputs, events, allocation/store counters, resource and arithmetic
refusal, and factory controls. Concurrent test output can interleave records
and is not a valid transcript. Historical schema-1 logs are not accepted.

Unlike capture replay, these raw logs have no producer receipt binding the
transcript to a source or executable. The supplied source and worker hash are
explicitly **caller provenance annotations**, not log-to-binary authentication
or independent Source-to-Plan validation. The output uses separate
`caller_supplied_*` fields. Nine semantic corruption controls check the
transcript's internal claims; neither command establishes timing or release
acceptance. Both accept an exclusive `--output PATH`.
