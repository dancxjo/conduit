# Two real wakes among 10,000 admitted nodes

Local production-kernel conformance for #5231, captured from clean source
`97175e74d` (full identity and harness digest retained in the receipt). This
extends the 10,000-node ready-set unit vector to actual FixedScheduler topology
admission, queues, remote ingress, Back invocation and Sign/value ownership.

The topology has 10,000 nodes, 10,000 independent one-slot ingress cords, four
fixed value slots, and 64 fixed Sign slots with eight remote-lifecycle slots.
All nodes first reach Await through ordinary Steps. Two inputs then wake nodes
63 and 9,999. A second input to each full queue refuses as Full without changing
its sequence or ownership. Four subsequent logical Steps consume both inputs
in deterministic fair order and return both nodes to Await/Idle. Every other
node remains asleep; all queues and value storage are empty afterward.

| Observation | Packed selector | Linear reference |
|---|---:|---:|
| Logical Steps after wake | 4 | 4 |
| Candidate node probes | 4 | 30,000 |
| Readiness-byte probes | 480 | 0 (node scan instead) |
| Allocation / reallocation calls in ingress-and-advance phase | 0 / 0 | 0 / 0 |
| Instrumented scheduler object bytes | 5,623,424 | 5,623,424 |

Both runs retain identical complete surviving Sign text, remote lifecycle
records, retention gap, receiver identities and terminal/storage result.
Startup fuel observations retire under ordinary retention with an explicit
bounded gap; mandatory lifecycle evidence remains. Each receiver checks the
exact input byte; the test also verifies two successful stores/copy bytes,
two remote-admission records, two ValueConsumed records, and no ready flags left.
This is stronger than matching final outputs alone.

The production topology and storage are fixed; preparation remains roughly
quadratic in cord count under current validation. Recorded debug preparation
and advance wall durations are diagnostic samples, not throughput, release
speedup or hard-realtime evidence. The test uses a native diagnostic thread
with a 64 MiB stack to accommodate construction of the large fixed object.
That stack is outside the measured execution phase and is not a constrained-
device reservation or physical-device proof. No runtime implementation changed
for this proof. This is direct admitted kernel topology, not an authored checked
Plot/Plan/std Host journey, FARGAN, browser or physical execution.

## Reproduce and inspect

From a clean committed checkout and a fresh output directory:

```sh
python3 proof/performance/large-sparse-replay.py --output /tmp/ten-thousand-sparse
```

The driver rebuilds the measured kernel package for each selector, runs the
exact fixed-only test, checks unchanged source before/after, retains executable
hashes and compiler profiles, and compares the entire replay. Post-measurement
formatting/serialization is outside the allocation sample. The seven artifact
hashes include both test logs, package-clean logs, and the report. Failed runs
retain logs with success=false. Raw files and the hash manifest are archived in
`evidence/ten-thousand-sparse-replay-raw.tar.gz`; the checked receipt is
`evidence/ten-thousand-sparse-replay.json`.

Package-native cleanup reclaimed 3.5 GiB of disposable old kernel test outputs
before rebuilding. Source, branches, active evidence and services were preserved;
only the main worktree exists. Parent #5231 remains open for its other Host-call,
large-graph performance and acceptance obligations. No CI was dispatched or awaited.

Local validation: both source-pinned fixed-only selector runs and seven artifact
hash checks passed. Alloc and linear-ready-reference kernel suites each passed
201 tests; formatting, strict alloc all-target kernel Clippy, and diff whitespace
checks passed locally.
