# Browser encounter preparation budget

Issue #6283 retains two independent empty-output durable Host readiness failures:
Integration 39934435057 and #6284 Candidate 39945464381. The latter passed the
other 268 browser cases. A later accepted Integration passed the same fixture;
that does not identify the native startup phase responsible for either failure.

The rendezvous fixture now spends one absolute encounter budget beginning in
its first beforeEach hook. Readiness no longer has an independent ten-second
cutoff. The existing Playwright 20-second deadline, one worker, zero retries,
actual Host/rendezvous processes and all admission assertions stay unchanged.
Late readiness refuses even if its timer callback has not yet run. Each phase
records elapsed time and outcome; process diagnostics retain PID, exit/signal
and at most 16 KiB of combined output with explicit truncation. Successful and
failed runs retain the on-disk JSON record and hashes of the actual fixture
helpers. A bounded summary without raw process output also reaches the CI log,
since successful runs do not upload the test-results directory.

Per-test ownership retires synchronously, cancels browser preparation, joins
pending preparation and late cleanup, and prevents a late Host spawn. Cleanup
is idempotent; process retirement keeps the existing TERM/KILL and five-second
exit bound, and teardown joining refuses after six seconds. Cleanup errors
remain alongside the original test failure. A stuck preparation remains an
explicit failed teardown, never a successful admission.

Twelve deterministic process/lifecycle controls plus the existing twenty
encounter/rendezvous controls pass. The focused pinned Chromium encounter also
passes with the retained historical CLI/product (10.3 seconds runner elapsed,
9.014 seconds encounter elapsed, Host preparation 112.6 ms and browser
preparation 7.711 seconds). This is a staged-product diagnostic, not a fresh
combined-source build or evidence that native startup actually exceeded ten
seconds. Fresh successor Candidate, normal merge and relevant accepted browser
Integration remain required. No production Host or parser behavior changes in
this repair.
