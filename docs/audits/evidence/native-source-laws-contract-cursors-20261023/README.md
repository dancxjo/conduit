# Prepared Native contract occurrences: current-source proof

Issue #6064. Measured source `678d6aa1027d9fd5592f11049a57aeadfb941793`, based on dev `f0dbc3484878e7325382f956da92e942ee039785`. The manifest pins thirteen source/test/suite files. A following evidence-only commit does not alter those files.

The supported `cargo +stable xtask check native-source-laws` suite passed all 17 tests: core projection 3, Native admission 6, actual occurrence/work 5, optional 2, and actual four-slot Beam 1. Formatting and unrestricted owning core/plot/Beam strict Clippy passed. Exact expanded dispatcher commands, elapsed times and lossless raw-log hashes are retained in `current-checks.json`. The original historical packet remains bound to its own source.

Before this fix, a nested primitive contract failure at row 1 / cell 1 rejected with empty occurrence indexes. The original production source and test-only patch are retained beside its actual failing log. That test now reports the exact source contract path and indexes. Separate controls prove one actual primitive check per step, finite total Native/contract occurrence pressure, cancellation, retained terminal cause, optional absence, reuse, identical prepared capacities, and zero calling-thread allocation/reallocation/deallocation over measured operations.

Every inherited contract is compiled into an exact schema-checked path and finite cursor before Play. Invariants still use their independent Native occurrence path. Both phases must complete before an opaque witness is available. Canonical validation/projection retain finite envelope scans: this is not constant-time byte decoding. Preparation and terminal destruction are outside measured Play; this is not whole-process memory proof.

The prior baseline passed 15 tests and strict checks before the missing regression was added. Those greens did not prove precise primitive-contract occurrence evidence or cooperative contract granularity. The reproduced negative and its fix are additional acceptance, not reinterpretation of that older proof.

Generated Rust materialization, installed Current/controller authority and hostile-code confinement remain separate. Candidate, merge, relevant Integration and the parent's shared publication batch remain required; this packet is local proof, not closure or stable acceptance.
