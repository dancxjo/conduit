# Constant-time node completion checks

Development slice of #5231, measured before changing terminal bookkeeping.

Ready selection can skip sleeping nodes while terminal detection still rescans their completion flags. When the final active node remains live after every earlier node completes, each Step and Idle poll inspected the whole active completion prefix. The new production-kernel fixture first reproduced 17*N flag probes for eight yielding Steps, one Await Step and eight Idle polls at N=1/8/32/256. Cooperative yields exhaust the exact Plan fuel grant; no readiness flags are seeded.

Both normal and abnormal completion now call one private membership transition. It sets the original completed flag and increments a fixed usize count only on the first transition. Both fixed and boxed preparation initialize the count to zero. The two execution-path completion checks compare that count with the immutable active-node count. Global cancellation remains a separate state and keeps the flags/count consistent. No readiness policy, Cord, value, Sign, fuel or Host Call behavior changes.

Completion queries now use one scalar comparison rather than an active-node scan. Normal/abnormal completion adds one fixed membership check and increment; no allocation, dynamic storage, queue or retry is introduced. Native instrumented objects grow by eight bytes at every measured capacity. This is a source/layout-specific observation, not a portable enclosing-size guarantee.

## Source-pinned production observations

Before source `b45c2c214b0cc31753902ab2cb39957249339efa`; after source `2122bd70b04df16ec4ae9bd16a16cae1b9f3caa2`. The same optimized fixture runs eight trials per shape in each of three independent processes on either side. All 96 paired samples retain exact complete Sign text, zero allocations/reallocations, matching shape/trial identities and matching recorded non-timing fields apart from the removed flag probes and eight-byte object growth. Baseline probes are 17/136/544/4352; afterward they are zero.

| Nodes | Before mean ns (process-mean range) | After mean ns (process-mean range) | Instrumented bytes before / after |
|---:|---:|---:|---:|
| 1 | 828.3 (569.6–960.0) | 775.7 (565.2–893.0) | 91152 / 91160 |
| 8 | 902.6 (621.0–1044.5) | 844.3 (637.0–953.1) | 93768 / 93776 |
| 32 | 1264.4 (861.5–1518.1) | 884.7 (629.4–1025.4) | 102744 / 102752 |
| 256 | 4132.2 (2726.4–5011.8) | 1002.1 (733.4–1147.8) | 186520 / 186528 |

The interval includes nine asserted Steps and eight asserted Idle calls, with kernel instrumentation and mandatory Signs. It excludes preparation, startup, final cancellation/retirement, thread creation and output serialization. This fixture produces no output values; these durations are not per-useful-output throughput. CPU/RSS records cover each entire process, not isolated Steps. The native fixture uses an 8 MiB diagnostic thread stack. Shared-host load/core/frequency are uncontrolled, so process variation is descriptive. The overlapping tiny ranges do not establish timing equivalence or a calibrated regression threshold; the deterministic eliminated scan is the primary result.

## Semantic and evidence checks

The added active-prefix test completes three nodes inside an eight-node reservation, verifies each count transition, repeated Drained results, duplicate notification suppression and untouched padding, then retires the scheduler. Boxed preparation separately checks initialization and exact completion. Existing normal/abnormal terminal Sign-refusal tests now verify no count update before admission and exactly one after successful retry. Quiescence tests which explicitly seed completed membership use the same private transition. Existing all-target kernel suites retain their pressure, cancellation, Host Call, remote, terminal and fairness obligations.

The retained verifier checks all artifact hashes, all 96 unique process/shape/trial keys, raw-log/report agreement, independently recomputed timing summaries and exact cross-source Sign/work replay. Four mutations are refused: a wrong artifact hash, duplicate sample, rehashed raw/report mismatch and a consistently rehashed altered Sign trace. Original captures stay unchanged.

The reference candidate enumerator remains the existing round-robin diagnostic; this optimization does not alter either selector. The remaining terminal Cord-emptiness scan runs after all nodes complete, and unresolved abnormal lookup still occurs at Drained. Those are not measured by this zero-active-Cord fixture and are not claimed constant-time here. Whole checked Host/Plan/Play, FARGAN, browser, ConduitOS and stable acceptance remain separate parent requirements.

## Reproduction and retained artifacts

The baseline instrumentation and candidate sources remain in this PR ancestry. From each exact clean source, build the kernel release test executable and invoke `scheduler::scan_characterization::terminal_completion::capture_completion_prefix --exact --ignored --nocapture --test-threads=1` in three processes. Build/profile commands, compiler, executable hashes, source identities, raw rows and process resources are retained in the archive. The retained capture helper records the exact development invocation; ordinary repository validation enters through `cargo xtask`.

After extracting the raw archive, verify it with:

```sh
python3 proof/performance/verify-terminal-completion.py --before /tmp/terminal-before --after /tmp/terminal-after --output /tmp/terminal-verified.json
```

Evidence: `evidence/terminal-completion-20261017.json`, `evidence/terminal-completion-negative-20261017.json`, and `evidence/terminal-completion-20261017-raw.tar.gz`. No physical timing, preemption, SMP, FARGAN, stable-release or full-parent closure claim is made.

Local validation: both alloc and linear-ready-reference release all-target suites pass 243 tests each. Strict alloc release all-target kernel Clippy, portable no-default-features checking, workspace formatting and diff whitespace checks pass. The six capture processes retain 96 exact paired replays; 24 raw artifact hashes verify. These are local checks; Candidate, combined integration and stable publication remain separate.


## Target contracts and parked legal feedback

The supported `cargo xtask make conduitos timing-profile` proof passes at `d395594ce9eb708129884fd191223bc5842091e4`: admitted deterministic cost 730 µs under a 1,000 µs deadline, observed met case 480 µs, planning refusal below the admitted basis, and distinct deadline-miss, timer/base-loss, cancellation and stale-basis outcomes. It labels the proof `deterministic-emulator` and `physical_claim=false`; these are not measured physical target latencies. The existing integration suites also pass seven retained-source protocol tests and five timing-profile tests, including the strict path's zero-allocation assertion.

At `45db0a216bc7344d5ad215464d10734915d1daf9`, the existing legal state/seeded feedback test now compares exact kernel Sign snapshots around 128 polls while a fore output remains pressured. No additional Gear-execution evidence appears; the output's generation and bytes stay unchanged. Releasing the output permits all 64 generations to drain in order to a normal semantic terminal. The strengthened test passes. Allocating inspection snapshots occur outside the compared kernel Steps. All recorded kernel/timing runtime source hashes still match the supported timing proof; the intervening change adds only this conformance assertion.

The receipt `evidence/scheduler-target-contracts-20261017.json` and complete raw archive with the same prefix retain command output, source identities and hashes. All six archived artifact hashes verify. This advances local compatibility and the no-spinning legal feedback requirement; browser/std whole-suite Candidate, combined development, stable and FARGAN acceptance remain separate.
