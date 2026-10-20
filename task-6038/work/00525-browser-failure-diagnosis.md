# 00525 browser Integration failure diagnosis

Observed 2026-10-20 22:39 UTC. Read-only diagnosis; no workflow rerun, cancellation, source edit, or timeout change.

## Exact failure

Integration 39500212975, source `00525faf30522b6851d226613e61e277f2b48f43`, browser job 118898476028 failed. Every unit and other target succeeded. Full log: `outputs/ci-resumed-20261020/observation-2239/118898476028.log`; retained diagnostics artifact 12304194618 (51,735,955 bytes).

All failures are in Chromium batch ci-1 (three slow files, 53 cases):

| Case | Actual failure |
| --- | --- |
| static-body-application.spec.mjs:56 | Entire test exceeded 180,000 ms; log identifies no final operation. Trace required. |
| workspace-arrival.spec.mjs:24 | Final reload line 86 expected Lulled, received Preparing for 5,000 ms. Earlier reload explicitly calls waitForWorkspacePreparation; final reload does not. |
| workspace-arrival.spec.mjs:91 | Test exceeded 20,000 ms; line 110 expected Stopped, received empty play-state. This case retains the default short whole-test bound despite starting/preparing real Body runtime. |
| workspace-membership.spec.mjs:529 | Entire test exceeded 20,000 ms; exact interrupted operation absent from log. Starts actual installed host plus real browser Body; trace required. |

ci-1: 20 passed, four failed, 29 did not run, 708 seconds. ci-2: all 210 passed, 1063 seconds. The 29 not-run cases followed the failed first test in the serial Handbook group; they are not positive acceptance.

## Shared scheduling change and isolation

PR #6163, merged 20:33:16 UTC, introduced default two-batch proof (commit 79c25aa61). `tools/ci/pipeline/targets/browser.mjs` spawns both concurrently through Promise.allSettled. Each process has one worker and zero retries, distinct config-server ports 4173/4174 and output dirs ci-1/ci-2. Specs partition exactly; neither batch may publish alone. `startStaticProduct` uses ephemeral per-test ports. Installed Host state uses mkdtemp. Persistent profile paths use testInfo.outputPath. No concrete shared-port, profile, output-directory, or state-file collision was found in this bounded inspection.

`docs/contributing/ci.md` lines 145-166 explicitly describe this as an experiment, warn about same-runner CPU/memory contention, and require one-batch fallback if concurrency interferes. Two independent one-worker processes still consume two Chromium workers in aggregate. CI batches overlap heavy real Source preparation with CPU-bound exact-quantity/WASM tests. All four observed failures are preparation/whole-test timing failures, but common resource contention is an inference, not established root cause. There are also independent readiness omissions in arrival tests; the first Handbook and membership failures need traces to distinguish these.

## Smallest safe next action

1. Root inspect the four retained traces for interrupted operation, pending preparation, browser errors and timing. Preserve the failure.
2. Contain the unaccepted concurrency experiment by restoring the single-batch default (or reverting the bounded scheduling change), with no test removal, timeout increase or retry. Keep exact spec union and zero retries. This restores the known serial proof contract while retaining the experiment as explicit opt-in if useful. Claim containment, not a proven product fix.
3. If traces confirm ordinary asynchronous preparation rather than fatal product error, add semantic readiness waits at the exact missing transition boundaries. Do not broadly relax assertions or use action-performing polling. Any whole-test budget change needs measured contract justification, not a passing retry.
4. Obtain fresh relevant Candidate and exhaustive Integration for the correction before admitting font PR #6165; its narrow native Candidate does not repair browser dev health.

## Ownership and overlap

CI scheduling change belongs to merged #6163 (browser pipeline/config/CI guide). Open PRs observed during diagnosis: #6165 native font; #6166 deferred native arrival; #6169 categorical request custody. No open browser scheduler repair was visible. Root integration owner owns CI files; this observer made no changes. Do not absorb #6166/#6169 scope. Current font Candidate remains independently running and is not evidence of repaired browser Integration.

## Retained trace inspection (22:49 UTC)

Root verified outer archive SHA256 `5f794a8f52bc8d1b9b39b370471f6e92a33757db15ed2e40eea1d66306d269cc`. Inspected nested JSON traces directly without running Chromium.

- **Handbook:** third persistent browser context successfully navigated to `/handbook/` at trace time167315–167422ms. Its last pending browser operation is `Expect toBeEnabled` on the exact **Lull** button, configured30,000ms, begun167434ms. The entire test180,000ms budget expired first. Prior context snapshots were acknowledged, including an Awake/playing snapshot; this is specifically waiting for application startup in the reopened profile, not evidence of wrong Body identity or missing persisted data.
- **Arrival/reload:** Lull click and Lulled assertion both passed before final reload. `__conduitWorkspace.settled()` completed in3.8ms; reload completed58.8ms. Final play-state assertion ran5058ms and observed Preparing. This establishes the missing post-reload readiness barrier precisely; no prior Lull failure.
- **Failed-save cancellation:** Birth button click consumed12,402ms; wake-button click was pending at browser-protocol level when the20,000ms whole-test deadline expired. The subsequent Stopped assertion was entered by the test wrapper with essentially no budget. The injected failed-save terminal cancellation had not been observed; this trace does not show an incorrect terminal state or an emitted effect.
- **Membership:** joining succeeded: two member cards and exact returned Source membership were observed. After reload, parts/hosts click took4787ms; expected two cards passed; final evidence evaluation returned; BOTH final assertions (`current` member count1 and current offers count1) passed. Timeout was reported after those assertions, with cleanup then closing the browser context. No failing domain assertion exists in the trace. The test awaits installed-host cleanup in `finally`; trace does not instrument that native process wait, so the precise timed-out cleanup await remains unproven. Do not call this a join refusal or wrong membership result.

No error-level browser event was found in the four trace streams during this bounded inspection. These traces strengthen the readiness/resource-pressure hypothesis and refute treating all four as independently demonstrated semantic product regressions. They still do not measure CPU contention or establish concurrent batching as the unique cause. Serial-default repair remains containment; fresh complete proof must establish corrected execution.
