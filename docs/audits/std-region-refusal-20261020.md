# Std execution-region refusal (#5563)

**Partial foundation; #5563 remains open.** Before source
`821abe1f388aa14e6ab8bd98d66008395a4fb677`, the installed Std runner could execute
a correctly sealed Plan containing an uninstalled execution region. Its ordinary
resource admission checked placements but ignored region-only lane resources and
preemption/isolation requirements. A sealed identity describes the requested
contract; it does not install the worker that can honor it.

The shared Std exact-profile boundary now refuses all nonempty execution-region
requests before reservations commit or Play starts. This runner has no installed
region/lane backend. No lanes or isolation capabilities are newly advertised.
Ordinary Plans without execution regions keep their installed execution path.
The separate ConduitOS region implementation is unchanged.

The regression uses a real checked baseline-32 expression and genuine installed
Back. It reseals otherwise valid Plans with one-, two- and four-lane requirements,
with and without preemption/isolation, then submits each through
`StdHost::run_external_plot_to`. All six requests refuse, with no delivery. The
unchanged reference then completes correctly with Play sequence zero, proving the
refusals did not consume a Play. The pre-fix run actually completed the first
unsupported request; that complete failure record is retained rather than
relabeled as a passing test.

The supported `cargo xtask prove resource-frame` entrance includes the new
regression. On clean unchanged source `821abe1f3`, it passed **47 actual tests**,
with no failures or ignored tests; zero-test filtered binaries are not counted.
The focused regression plus seven replacement tests also pass. Strict Clippy
for the changed Std library/test and focused formatting pass. Full commands,
complete compressed before/after logs and SHA-256 hashes are retained in the
[validation record](evidence/std-region-refusal-20261020/validation.json).

This proves refusal of unsupported contracts, not physical parallel execution.
Still required by #5563: immutable region membership, actual current one/two/four
lane admission/reservation, finite worker/mailbox/scratch preparation, typed
cross-region transfers, retained source/effect ownership and physical-host
admission evidence. #5564 separately owns simultaneous progress and worker
failure/cancellation/commit proof. The current core cooperative region verifier
requires connected placements to share a region, while Std's resource ledger
only reserves placement resources. Neither is a completed hosted lane backend.
Future support must validate and reserve region requirements explicitly and
preserve the existing kernel's authority over readiness, fuel, pressure, effects
and commit; removing this refusal alone cannot provide support.

Direct dev delivery follows the user's explicit workflow bypass. Stable
publication and the remaining performance parent acceptance stay in #6078.
