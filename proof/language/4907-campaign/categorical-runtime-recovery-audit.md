# Categorical runtime recovery audit

Read-only source/commit audit against origin/dev `9ff334927f546c7f9a4a241ef38b548bd1b25549`. No transplant, build, inference, or GitHub observation performed.

## Smallest useful prerequisite

Recover a focused **generic AI numerical request custody** PR, not the held Language chain:

1. `73defbe38`: split existing Back/operation factory into back.rs and owner.rs; add bounded invocation.rs outlet with private non-cloneable committed tokens and exact bytes/Plan/profile custody; include invocation_cases test module.
2. `c3cda235c`: required reservation release on failed input/fuel and concurrent reader/held-slot tests. Do not omit this correctness repair.
3. `821d22607`: required exact outlet membership check, including equal-value foreign outlet negative test.
4. `8ce3f9b1c`: channel.rs and module wiring, actual ordinary KernelCompositeHost ingress/output ACK path and generic lease retention, plus channel_cases.
5. Apply relevant `3ce4b81d3` lint/ownership-preserving hunks to this non-canonical channel (its prepare_canonical hunk is absent by design).
6. Apply `395b52b9b` Sign storage admission, available_slots accessor, full32 real kernel/held-token/no-heap test and lint fixes in invocation/concurrent tests. Translate offsets, not semantics.

The parent of 73defbe38 has **zero diff from current dev** for integer_categorical_step.rs and its root test fixture. Existing manifest already has kernel-operation-owners, composite/plan-lowering dependencies and needed generic test dependencies. No AI Cargo.toml addition or lockfile change should be needed. Current root's added Speech/Language/Audio dev-dependencies belong to other held work and must stay out.

No architecture/kernel changes are required. Current root differs from dev by deleting prepared_session infrastructure: those deletions are unrelated and must not be recovered. Existing dev StepBack already has prepared_output/step_committed; existing composite has prepare_with_sign_storage/KernelCompositeSignStorage. Numeric channel uses ordinary existing production kernel; no replacement scheduler or rearm mechanism is needed.

## Canonical Source is a separate dependency

`c9cdfefee` is NOT necessary to compile or test the generic authored CheckedPlot channel from 8ce. It adds canonical_export.rs (337 lines), composite module wiring, prepare_canonical and two Language full-model fixture/test files. Do not import its Language files merely to get a generic AI prerequisite merged.

The initial generic channel's prepare(CheckedPlot, definition, ...) rebuilds from actual supplied authored export and compares the full definition. It verifies exact selected numerical placement/resource/model, private outlet membership, actual ingress sequence, exact input and output bytes, actual output ACK, immutable sealed Plan and active Plays. Caller lease L is opaque **metadata**, not authenticated parser authority. Manual StepBack invocation tests are protocol diagnostics; channel_cases supplies the real kernel proof. A borrowed exported definition or equal numeric bytes alone are not authenticated checked Language Source execution or a committed parser transition.

Before a downstream canonical Language consumer is published, recover c9's generic composite adapter in a separate focused slice (or explicitly expand this PR) with generic canonical-source tests. It must bind actual checked document/entry/catalog to sealed source/checked/expanded identities, graph, Fore queue/typed contracts and declared ports. Current canonical source rejects unsupported startup/resource/abnormal Fores, activation/shared-pool/state/fusion/region constructs, fanout and non-payload tracks. Preserve these refusal boundaries. The existing c9 tests live in Language and use 413/76/25 parser profile; replace/extract generic canonical tests rather than importing Language policy into the AI prerequisite. Retain wide payload coverage beyond the incidental 64B toy bound and re-sealed foreign/missing/extra/queue/contract/track/gear/cord negative cases.

## Entrances and proof

There is currently no categorical-specific xtask suite (parser-scorer and model-inference suites do not name integer_categorical_step). Add a small public `cargo +stable xtask check categorical-runtime` suite through the existing CheckSuite/CLI/Step pattern; owner should choose final name. It should run conduit-ai integer_categorical_step with kernel-operation-owners, one test thread, and no model fitting/download. Root owns xtask wiring. Raw Cargo/test binaries remain internal conveniences.

Required focused proof: whole integer_categorical_step target, including existing inference/placement/scheduler behavior; invocation pre-ACK opacity, exact bytes and outlet membership, cancellation/bad input/fuel reservation release, one-winner concurrent readers and held-token pressure/reuse; actual channel distinct leases for equal inputs, malformed/foreign definition, busy/pressure custody, terminal cancel/pending recovery; full32 submit/ACK with all tokens held, all four remote lifecycle kinds per invocation, 33rd Pressure with unchanged lease and alloc/realloc/dealloc zero. Test default/no-owner build feature compatibility plus owner feature Clippy with denied warnings and formatting, then exact-head Candidate/integration per acceptance policy. Atomics gate ptr+8; do not promote hosted concurrency proof to other targets.

Sign storage is finite: 16 additional local and 4 remote events per admitted outlet slot, bounded by <=128 slots and 1MiB outlet storage. Tokens may be dropped/recycled but this does not promise unlimited historical Sign lifetime. available_slots is an observation, not a reservation; submit/kernel admission remains authority. Existing root full32 proof can be source-bound reused where exact relevant code/fixtures/dependencies match, with final exact-head CI; do not rerun the speech suite for this slice.

This prerequisite alone does not prove installed Language Current authority, same-epoch parser control, lexical materialization, commit/prosody realization, or parent #4907 completion.
