# Retained #5395 evidence and replay

Read [the final report](runtime-performance-5395-final.md) for conclusions,
clock/allocator/copy scope and uncertainty, and [the acceptance audit](runtime-performance-5395-closure.md)
for the requirement mapping. All new measurements remain attributed to clean
`a52ddeff5fc3e047bdf311591a1c7ef8233d69ee`. Later publication changes do not
rewrite that Source.

The `evidence/runtime-5395-20261017-*` files contain:

- Four `hosted-runtime-domain-20261017-0N.tar.gz` immutable captures: conformance,
  metadata, status with checksums, raw samples, reports and frozen authored
  Source, inputs, Host, Plan and canonical output receipts. `01` is diagnostic;
  `02`–`04` form the independently observed process cohort.
- `runtime-domain-cohort-comparison.json`: independent verification of all three
  cohorts, original manifest hashes, all batch samples/distributions and process
  comparisons; `runtime-domain-expanded-cohort.json`: all actual Play identities,
  cross-process Source/output parity and derived std boundary-copy extents.
- `runtime-domain-cohort-control.json` and `runtime-domain-cohort-workers.json`:
  retained affinity policy and live `/proc` PID/start/executable observations.
- `runtime-domain-verification-01.json` and `runtime-domain-negative-01.json`:
  diagnostic verification and ten independently refused rehashed corruptions.
- `kernel-domain-current-20261017.tar.gz` and `sign-domain-current-20261017.tar.gz`:
  current-source instrumented kernel byte-sink and production Sign-log probes,
  raw process logs/resources/build metadata, samples and hash manifests.
  `attribution-verification.json` independently joins their raw rows and summaries.
  The larger `kernel-domain-current-verification.json` and
  `sign-domain-current-verification.json` retain the earlier full inspection.
- `runtime-domain-local-checks.log`, `runtime-domain-test-first-failure.log`,
  command logs: passing local checks plus the initial missing allocator-test
  mutex and routine Clippy failures. No failure was represented as successful
  performance evidence. An empty kernel wrapper log is expected: that driver
  writes into its capture directory, not stdout.
- `manifest.json`: SHA-256 and byte sizes of the above retained files and all
  proof/analysis helper sources. Archive member bytes were roundtrip compared
  to the original local directories before publication.

Archives contain relative paths with one capture directory root. Extract into
a new scratch directory; do not modify their raw data/status to relocate the
original absolute provenance paths. The comparison helper uses the original
status output path to join the retained live observation, so relocation works.
For example, after extracting all archives into `/absolute/replay`:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 proof/performance/compare-runtime-processes.py \
  --capture /absolute/replay/hosted-runtime-domain-20261017-02 \
  --capture /absolute/replay/hosted-runtime-domain-20261017-03 \
  --capture /absolute/replay/hosted-runtime-domain-20261017-04 \
  --control docs/audits/evidence/runtime-5395-20261017-runtime-domain-cohort-control.json \
  --process-observations docs/audits/evidence/runtime-5395-20261017-runtime-domain-cohort-workers.json \
  --output /absolute/replay/comparison.json

PYTHONDONTWRITEBYTECODE=1 python3 proof/performance/analyze-runtime-domain-cohort.py \
  --verified-comparison /absolute/replay/comparison.json \
  --capture /absolute/replay/hosted-runtime-domain-20261017-02 \
  --capture /absolute/replay/hosted-runtime-domain-20261017-03 \
  --capture /absolute/replay/hosted-runtime-domain-20261017-04 \
  --control docs/audits/evidence/runtime-5395-20261017-runtime-domain-cohort-control.json \
  --workers docs/audits/evidence/runtime-5395-20261017-runtime-domain-cohort-workers.json \
  --source-root /absolute/conduit --output /absolute/replay/expanded.json

PYTHONDONTWRITEBYTECODE=1 python3 proof/performance/check-runtime-domain-custody.py \
  --capture /absolute/replay/hosted-runtime-domain-20261017-01 \
  --verifier proof/performance/verify-runtime-capture.py \
  --output /absolute/replay/negative.json

PYTHONDONTWRITEBYTECODE=1 python3 proof/performance/verify-runtime-attribution.py \
  --kernel /absolute/replay/kernel-domain-current-20261017 \
  --sign /absolute/replay/sign-domain-current-20261017 \
  --output /absolute/replay/attribution.json
```

Use normal Python assertions (never `-O`). `--source-root` is a repository with
measured commit available: copy-extent source checks use `git show` that commit,
not whatever happens to be checked out. The expanded analysis consumes the
preceding independent verification receipt and rehashes all referenced data;
it does not substitute for running the primary verifier.

The single product measurement entrance remains `cargo +stable xtask prove
runtime-performance --hosted-pipeline true`; the report gives its full command.
The kernel/Sign drivers are internal attribution helpers, not alternative
product execution paths. Their recorded commands were:

```sh
taskset -c 4 python3 proof/performance/kernel-overhead.py \
  --output /absolute/new/kernel-capture --runs 3 --include-large
taskset -c 4 python3 proof/performance/sign-record-cost.py \
  --output /absolute/new/sign-capture
```

Existing immutable metadata's then-pending follow-on notes remain untouched.
The expanded cohort, source-bound copy accounting, fresh Sign/kernel probes,
final report and explicitly scoped subsequent browser gate resolve those
native reporting items. Reviewed thresholds and matched browser/WASM are still
future work, as the parent explicitly permits; they are not claimed completed.
