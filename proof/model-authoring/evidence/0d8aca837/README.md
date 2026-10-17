# Verified hosted library revision 0d8aca837

These proofs ran against clean source
`0d8aca837a2a83d53fab010bf150e25db1547910`. Both producers report unchanged source
and `working_tree_dirty: false`. The later evidence/documentation commit changes
no model behavior. [Inventory](inventory.json) identifies every retained output.

| Proof | Class | Result |
|---|---|---|
| [Extracted tutorial command](cpu-docs/manifest.json) | deterministic-unit | The actual marked `cargo xtask` command passed |
| [CPU library contracts](cpu-docs/command/manifest.json) | deterministic-unit | 29 runtime tests and four Rust documentation contracts passed; proposed voice example ignored |
| [CPU retained journey](cpu-docs/command/artifacts/journey.json) | synthetic regression fixture | Loss 9.0 → 0.004003; fresh-runtime resume at step 40, continuation through step 80, inference-only reload |
| [CUDA](cuda/manifest.json) | physical-local-hardware | Explicit RTX 3060/CUDA 0 training, CPU comparisons, and fresh-runtime resume through step 81 passed |

CUDA [output](cuda/stdout.log) records loss 9.0 → 0.004025. Maximum absolute
output drift was 0.0005168915 for independently trained snapshots and 0.0003466606
for identical-checkpoint inference. Both satisfy the fixture's declared 0.001
bound. F32 storage does not imply bitwise or full-mantissa device-kernel equivalence.
These tolerances belong to this fixture, not every model or FARGAN profile.

The CPU artifacts retain separate immutable resume and inference stores. Their
six immutable members total 4,613 bytes; the inference descriptor plus weights
occupy 928 bytes. They are our synthetic regression weights. Build/data identities
inside the semantic fixture are explicitly synthetic; the producer separately
records actual source/build profile and the CUDA device evidence.

CPU stderr includes deliberately rejected inputs: a caught Burn tensor-rank
assertion during optimizer restoration and two expected Rust compile failures
for shared-reference publication. The suite returns success after verifying
explicit refusal and unchanged state. The [development records](../../development/README.md)
retain genuine earlier failures, including the rejected CUDA `1e-4` probe.

No ordinary Plot/HostCall execution, ConduitVoice, corpus cursor, natural-speech
reconstruction, attended listening, or stable issue acceptance is established.
