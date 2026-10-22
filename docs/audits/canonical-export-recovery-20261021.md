# Canonical checked export recovery

Prepared scope for [#6063](https://github.com/dancxjo/conduit/issues/6063), based on current `dev` `3672758cf3f8bbaec01dab14476882433d31621f`. This is one generic adapter recovery, not a campaign checkpoint merge or completion of #4907. No PR, CI run, build, or Cargo check was started during this preparation; the shared build cache remained with its existing owner.

## Reused implementation and bounded correction

`architecture/composite/src/canonical_export.rs` recovers the adapter from `c9cdfefee80fe5bce30c458b72ee0f6937f6e813`. The numerical `prepare_canonical` bridge is recovered into the current AI channel without replacing its already-present invocation custody and allocation-free refusal handling from the earlier `3ce4b81d3` work. No Language fixtures or dependencies are introduced into generic Composite or AI proof.

The adapter re-expands the supplied checked document and entry. It requires exact Source document, checked Plot and expanded Plot identities, completion/realization facts, declared Gear meaning, unique placement identity, ordered Cord endpoints and tracks, selected port/typed-contract correspondence, and every independent exported payload Fore. A fresh Plan digest alone does not supply this correspondence.

The held adapter checked typed maximum fit for exported Fores but not internal Cords. Recovery adds the analogous internal whole-payload requirement: a source output's validated typed maximum must fit the selected Cord byte capacity. A resealed 255-byte Cord for a 256-byte source contract must refuse even when Plan integrity still verifies. This does not prescribe queue counts: legitimate one-item and two-item allowances within selected limits remain Plan facts, and a positive regression retains the alternate allowance exactly.

The initial adapter explicitly refuses startup/resource/abnormal Fores, duplicate or fanned-out exported Fores, non-payload tracks, selected boundary Lines, unsupported pressure policy, multiple fragments, activations/preparations, shared pools, retained state, execution fusion/regions, resource ports and non-payload/resource/abnormal internal Cords. The ordinary kernel preparation still owns its fixed scheduler/resource limits. No fragmentation, fallback queue, identity rewriting, raw execution entrance, or Host-admission/currentness claim is added.

## Prepared proof surface

`architecture/composite/tests/canonical_export.rs` owns a generic checked Source with two byte-copy primitives, one real internal Cord, independently sealed typed input/output Fores and 256-byte selected bounds. Its positive execution uses the full admitted 256-byte payload through `KernelCompositeHost`, retaining exact output bytes, ACK sequence, Play identities and Signs. It does not use a parser model or a Language policy fixture.

Prepared negative cases cover foreign Source, all three Plan identities, missing/extra Fores, queue underflow/overflow, typed contracts, tracks, Gear identity/meaning/configuration, missing/extra Cords and wrong endpoints. Counters require integrity-valid Source/Fore/graph counterexamples rather than counting only stale seals or core integrity failures. Separate cases cover unsupported queue/abnormal profiles and checked input-Fore fanout.

The supported entrance is `cargo +stable xtask check canonical-export`: generic behavioral tests, focused strict Composite test lint and strict AI library lint with the numerical operation-owner feature. No Language dependency is needed for this check.

## Acceptance boundary

Formatting and `git diff --check` were performed during preparation. The tests, strict checks and supported entrance remain unexecuted until the integration owner runs the exact prepared source from its cache-owning path. Historical held Language-model evidence is not relabelled as proof of this generic recovery. #6062's dependency status, required current-base CI, exact-head acceptance, reviewed merge and the parent's shared Integration/publication batch remain delivery gates; this preparation does not close #6063.
