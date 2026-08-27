# CI impact planning benchmark

This note records the acceptance benchmark for issue [#1833]. It compares an
exhaustive pre-change run with three marker-only pull requests. The marker
commits changed comments or test descriptions without changing behavior, so
the measured workflow topology came only from ownership and dependency impact.

All final selective samples used `848201d9809afe6867817c5522190dde1cea6e12`
as their exact base. Wall time is workflow creation through completion. Runner
time is the sum of successful job durations, including the stable final gate.

| change class | exact run | selected work | wall time | runner time | target |
| --- | --- | --- | ---: | ---: | --- |
| pre-change exhaustive main | [33034313447] | 27 jobs; full machine matrix | 9m18s | 82m52s | baseline |
| ordinary Pete application | [33088341311] | Pete lint; Pete and `xtask` tests | 2m50s | 4m03s | met: under 3m |
| `conduit-kernel` | [33088347003] | all reverse-dependent tests and kernel-consuming machine claims | 15m12s | 77m14s | exception: 2–4m infeasible without dropping required claims |
| ESP32-C3-only firmware | [33088352455] | C3 firmware and universal guards | 4m51s | 3m56s | met: 2–5m |

The retained plans prove the selection rather than inferring it from skipped
jobs:

- The Pete plan changed `conduit-pete`, mechanically expanded tests to
  `conduit-pete` and `xtask`, selected only lint and product-test shards, and
  selected no ESP32, browser, Pico, portable, or ConduitOS proof.
- The C3 plan selected only target `c3`; the standalone firmware package is
  proved by its dedicated job and is intentionally not passed to root-workspace
  clippy.
- The kernel plan changed `conduit-kernel`, selected all six workspace shards,
  all three ESP32 targets, browser proof, all eight x86 proofs, all four
  architecture proofs, and the AArch64 product proof. This is required by the
  kernel-dependent-claims self-test and is not a conservative full fallback.

The kernel target band is not feasible with the current required proof set.
The run's critical `conduitos-x86-kernel` job alone took 7m39s, browser took
5m46s, and several additional exact machine claims ran independently. Reducing
that result to 2–4 minutes would require weakening the explicit requirement
that a kernel change expand to every kernel-dependent claim, or separately
making those proofs materially faster. Neither belongs to impact selection.

## Planning cost

The three retained `Plan exact CI impact` steps took 4s, 16s, and 4s. The 16s
kernel sample included a fresh dispatch compilation; even there, planning was
less than two percent of workflow wall time. The small dispatch boundary keeps
ordinary planning startup from compiling the full `xtask` dependency universe.

## Build and compilation economics

ConduitOS x86 jobs consume one exact immutable proof image built once by
`conduitos-proof-image`; the architecture and product jobs also reuse pinned
bootloader and tool bundles rather than independently acquiring them. Selected
PR jobs install only their required target toolchains.

Repository-wide `sccache` was evaluated and deferred. Existing Rust cache
restoration already covers third-party dependencies. The dominant waste was
over-selection and the former monolithic planner startup, addressed by exact
package/proof selection and the dispatch boundary. A shared compiler cache
would add remote compiler-artifact identity, toolchain/environment keying,
eviction, security, and reproducibility policy while not reducing mandatory
test or PLAY breadth. Revisit it only if post-selection compilation again
dominates representative required runs.

[#1833]: https://github.com/dancxjo/conduit/issues/1833
[33034313447]: https://github.com/dancxjo/conduit/actions/runs/33034313447
[33088341311]: https://github.com/dancxjo/conduit/actions/runs/33088341311
[33088347003]: https://github.com/dancxjo/conduit/actions/runs/33088347003
[33088352455]: https://github.com/dancxjo/conduit/actions/runs/33088352455
