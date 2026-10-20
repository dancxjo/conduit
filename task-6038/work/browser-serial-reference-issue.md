Blocks native typography admission #6165 / #5994. Follow-up to the two-batch experiment #6163.

Integration [39500212975](https://github.com/dancxjo/conduit/actions/runs/39500212975) at source 00525 failed browser proof while every unit/native/other platform lane passed. Chromium batch 1 recorded four failures, 29 cases not run and 20 passes; batch 2 passed all 210. Failed cases are static-body-application:56, workspace-arrival:24 and :91, and workspace-membership:529. The retained diagnostics artifact is 12304194618; its downloaded archive SHA256 is 5f794a8f52bc8d1b9b39b370471f6e92a33757db15ed2e40eea1d66306d269cc.

Traces/logs show timeouts and unfinished preparation in the slow batch. Resource contention remains an inference; no shared port/state collision has been established. The documented throughput experiment is not green and must not be treated as accepted delivery.

Scope: restore the single-batch default in the existing browser pipeline, preserving the exact checked spec inventory, sealed product, Chromium project, one worker, zero retries and evidence validators. Keep two-batch mode explicitly opt-in for controlled diagnosis. Update the experiment status in the CI guide. No timeout increases or product/test weakening.

Acceptance: focused pipeline checks; actual source-bound Candidate Chromium proof with one default batch and the complete reviewed case inventory; exact-head combined Integration green before resuming typography admission. Preserve the failed run and traces, and report any remaining product readiness failure distinctly. Work starts from current dev c93a4cca2 in codex/ci-single-chromium-reference-20261020; no other active browser repair PR was found.
