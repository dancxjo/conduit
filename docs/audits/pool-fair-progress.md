# Shared-pool fair progress and failed-member retirement

For #5551, accepted bounded waiting, requester turns and anti-starvation aging
are exercised together with ordinary atomic member occupation. A modeled
10 ms worker remains occupied while another requester progresses on the
100 ms worker, despite 31 queued requests from the active requester. This is
adversarial forecast/occupation evidence, not measured provider latency.
Pressure and cancellation preserve pinned members and waiting custody, and
stale epochs cannot trigger or release replacement work. Existing full-suite
queue limits, deterministic FIFO/ties and maximum-bypass aging tests remain.

The regression reproduced a missing hosted retirement entrance: provider loss
moves the kernel member into Releasing, while the old hosted `release` asks
for Active and returns `InvalidLifecycle`. `HostedSharedPool::complete_release`
now exposes the existing kernel completion transition for owner-confirmed
retirement. Loss alone retains occupation; the new call neither retries nor
replays work. Exact lifecycle/epoch checks reject active, repeated and stale
completion. Unrelated work uses the still-Ready sealed alternative; recovery
admits a fresh epoch without changing the Plan.

At source `68afcccbac2a3d62f15df0bc83ba276c4deaa92b`, all 46 plan-lowering tests across five
targets and all 12 related kernel integration tests across three targets passed.
Strict all-target plan-lowering Clippy and formatting passed. The initial
filtered kernel-library command selected zero tests and is not counted as
validation. Exact logs and bounds are retained in
[evidence/pool-fair-progress.json](evidence/pool-fair-progress.json).

This uses the actual generic hosted pool and kernel with modeled provider
facts and costs. No actual Ollama latency, two-machine execution, physical effect
completion or hostile-code confinement is claimed. Fair waiting cannot promise
completion from a provider that never retires its work. Parent #5228 remains open.
