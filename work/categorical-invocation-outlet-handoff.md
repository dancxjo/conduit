# Bounded categorical committed-invocation outlet preparation

Base 4b89e28f235d6ad6bc953e2daf222676074c9310. No builds, commits, runtime executions, or GitHub observations. Six-path complete patch and SHA256 manifest accompany this handoff; include new child files, not just tracked diff.

## API and ownership

Existing public `integer_categorical_step::{CategoricalStepBack,CategoricalStepReceipt}` and `owner::CategoricalOperationFactory` paths preserved. Coherent Back and factory extraction leaves parent407, Back170, factory118, outlet231 production lines. Existing tests unchanged except declaration of new child cases. No manifest/facade/DSP changes.

Opt-in `CategoricalOperationFactory::for_plan_with_invocation_outlets(plan,profiles,slots_per_placement)` returns factory and reader handles. Each factory outlet can attach once to exactly the selected full placement, same prepared profile allocation and flow/value flag. Failed descriptor/preparation admission does not claim the writer. A second prepare for that outlet refuses; every new Play must use a new factory/outlet. Old `for_plan` behavior preserved.

`CategoricalInvocationOutlet::try_take()` returns private-field non-Clone `CommittedCategoricalInvocation` only after private publication in the existing Back `step_committed`. Receipt getters retain exact sealed Plan, full selected placement, prepared admitted model/resource/profile, invocation, flow flag, and complete canonical input/output. Caller-sized copies avoid receipt decoding/encoding in the runtime path. Existing public Copy diagnostic receipt remains metadata, not opaque authority.

## Finite storage and concurrency

1..128 slots per placement; input+output+8-byte invocation payload times slot count <=1,048,576 bytes. `retained_payload_bytes` reports actual payload; this excludes atomic state bytes, Arc/slot metadata, immutable cloned Plan/placement/profile binding and allocator overhead. All allocation happens at preparation, following existing hosted factory/Back practice; no hidden step growth. Fixed canonical input/output byte arrays use AtomicU8; runtime only copies codec-established exact frames. Model inference and numeric policy remain unchanged.

State FREE -> RESERVED -> COMMITTED -> CLAIMED -> FREE. Exclusive writer reserved once. Reserve before inference, consume/send; StorageExhausted detail3106 before effects when held leases occupy all slots. No spinning, unsafe, mutex or scheduler. Reader Acquire claim observes Release publication; writer Acquire reserve observes Release lease drop. Relaxed byte copies occur only under exclusive reservation or immutable claimed lease. The published reservation disarms its Drop to prevent an ABA cleanup race with a rapidly released/reused slot. Invocation IDs monotonically advance in original Back and refuse u64 exhaustion. No slot generation token is exposed/reused: the opaque owned lease blocks reuse until Drop, and invocation identifies executions.

Outlet and all added Back fields gated target_has_atomic=ptr AND8; ordinary existing Back remains under original kernel-step feature gating. Existing module already used alloc::sync::Arc; no new atomic requirement is imposed on old paths. Outlet uses no AtomicU64. Explicit Send+Sync compile assertions for reader/receipt are in focused tests. Root must compile supported target configs; this is not cross-target proof.

## Prepared proof and limits

Two new child tests: acknowledgement-only publication, no step allocations, exact bytes/resource/Plan/placement, held-slot pressure and immutable retained receipt, reuse after lease Drop, cancellation, malformed input, finite capacity refusal, foreign placement refusal without writer loss, and second-instance refusal. These are direct StepBack protocol diagnostics, not installed Plan/Play acceptance. Original scheduler tests retained. Root admission still requires focused tests plus extraction-required Clippy/full-workspace checks per AGENTS, with genuine blockers recorded if applicable.

This outlet does not authenticate a caller-supplied Play ID, prove Language Source legality, produce feature/branch custody or rank/parser decisions, establish learned model quality, or provide installed target proof. A later Language adapter must consume opaque invocation evidence alongside exact private pending feature-query and generation custody; target runtime must retain actual Play relation. No arbitrary score-array authority is added.
