# Membership encounter readiness budget (#6283)

Accepted source `99dd81720a961598e2a1d453350153514770eb1b` failed
[Integration 39972384625](https://github.com/dancxjo/conduit/actions/runs/39972384625),
browser job 120490238731. The last membership Host-loss case failed the local
10-second durable-service readiness timer with empty output; the other 268
Chromium cases passed. This is a separate call site from the earlier Crèche
fixture repair. The original failure excerpt and full-log/artifact hashes are
retained in [the evidence packet](evidence/6283-membership-startup/receipt.json).
The complete failed log and downloaded diagnostics ZIP remain in thread
outputs; GitHub retains artifact 12514936800.

The membership fixture now uses the existing shared process-output and
preparation-state helpers. Its native service and rendezvous readiness spend
the enclosing test's absolute budget from the first hook. The Host-loss case
still has 20 seconds. The other Host-join case retains its existing cold-browser
budget; no test deadline, assertion, worker count or retry policy is enlarged.
Both independent preparation tasks retain ownership through retirement.
Directory ownership is registered before later preparation can complete, child
shutdown escalates within finite bounds, and bounded source-hashed PID/exit/
output/timing diagnostics survive successful and failed runs.

Seventeen shared readiness/cleanup controls pass, including readiness beyond
ten seconds inside the unchanged twenty-second encounter, late output refusal,
process failure and late-owned cleanup. A fresh native CLI built from the
current base and two real Chromium membership cases passed with the separately
retained historical browser product, one worker, zero retries. The Host-loss
case took 6243.95 ms overall: Host preparation 4482.69 ms and browser preparation
4778.26 ms. This local result does not reproduce the CI cold-start delay.
The packet records all three fixture/helper hashes, native CLI hash and the
complete retained-browser file manifest. It is a staged-product diagnostic,
not a fresh combined-source browser target pass.

The supported pipeline preflight passes workspace formatting, locked metadata,
generated definitions, standalone locks, 345 pipeline controls and workflow
lint. Initial local preflight attempts refused missing sparse-checkout evidence
fixtures; restoring the tracked evidence trees resolved that environment issue.
The local invocation was also corrected to use the actual full base commit ID.
No product or CI policy was changed to obtain this result.

Fresh Candidate and accepted dev browser Integration remain required. Internal
native startup phase attribution and #6283's broader diagnosis obligation remain
open; this fixture repair does not explain the earlier empty-output delays.
No environmental appliance feature changes are included in this repair.
