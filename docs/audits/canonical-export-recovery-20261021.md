# Canonical checked export recovery

Prepared scope for [#6063](https://github.com/dancxjo/conduit/issues/6063), based on current `dev` `3672758cf3f8bbaec01dab14476882433d31621f`. This is one generic adapter recovery, not a campaign checkpoint merge or completion of #4907. No PR or CI run was started during recovery. The shared build cache remained with its integration owner, who performed the first supported trial recorded below.

## Reused implementation and queue semantics

`architecture/composite/src/canonical_export.rs` recovers the adapter from `c9cdfefee80fe5bce30c458b72ee0f6937f6e813`. The numerical `prepare_canonical` bridge is recovered into the current AI channel without replacing its already-present invocation custody and allocation-free refusal handling from the earlier `3ce4b81d3` work. No Language fixtures or dependencies are introduced into generic Composite or AI proof.

The adapter re-expands the supplied checked document and entry. It requires exact Source document, checked Plot and expanded Plot identities, completion/realization facts, declared Gear meaning, unique placement identity, ordered Cord endpoints and tracks, selected port/typed-contract correspondence, and every independent exported payload Fore. A fresh Plan digest alone does not supply this correspondence.

Untested predecessor `1f0030152` added an internal source-maximum-versus-Cord-capacity rejection during preparation. Independent inspection disproved that premise: existing lowering explicitly takes `min(semantic_bound, byte_capacity)`, including its 256-byte typed / 64-byte queue regression. The recovery successor removes that extra restriction. A valid narrower queue is an exact admitted Plan fact, not a Source forgery or a promise to carry every value allowed by the broader semantic contract.

Prepared regressions retain a valid 255-byte internal Cord with a 256-byte source contract, inspect its lowered maximum of 255 bytes, assert the actual copy driver's `OutputBlocked` refusal and absent output for 256 bytes, and execute 255 bytes successfully in a fresh runtime. They do not substitute `SemanticValueBoundExceeded` observed at another kernel boundary. Legitimate one-item/two-item allowances also remain exact Plan facts, while zero or out-of-selected-ceiling queues refuse. Exported Fore typed-fit rules remain those of the recovered initial profile.

The initial adapter explicitly refuses startup/resource/abnormal Fores, duplicate or fanned-out exported Fores, non-payload tracks, selected boundary Lines, unsupported pressure policy, multiple fragments, activations/preparations, shared pools, retained state, execution fusion/regions, resource ports and non-payload/resource/abnormal internal Cords. The ordinary kernel preparation still owns its fixed scheduler/resource limits. No fragmentation, fallback queue, identity rewriting, raw execution entrance, or Host-admission/currentness claim is added.

## Prepared proof surface

`architecture/composite/tests/canonical_export.rs` owns a generic checked Source with two byte-copy primitives, one real internal Cord, independently sealed typed input/output Fores and 256-byte selected bounds. Its positive execution uses the full admitted 256-byte payload through `KernelCompositeHost`, retaining exact output bytes, ACK sequence, Play identities and Signs. It does not use a parser model or a Language policy fixture.

Prepared negative cases cover foreign Source, all three Plan identities, missing/extra Fores, queue underflow/overflow, typed contracts, tracks, Gear identity/meaning/configuration, missing/extra Cords and wrong endpoints. Counters require integrity-valid Fore/graph counterexamples rather than counting only stale seals or core integrity failures. Separate cases cover unsupported queue/abnormal profiles and checked input-Fore fanout.

The prepared AI regression independently constructs canonical checked Source from the installed generic small categorical profile and is intended to prove `prepare_canonical → submit → step → output ACK` through exact finite Fores. It checks moved request custody, exact canonical input/output, Plan/Play correlation, remote lifecycle Signs, held receipt pressure, and foreign Source/misbound-definition refusal. No parser policy or currentness authority is introduced.

The supported entrance is `cargo +stable xtask check canonical-export`: generic behavioral tests, the existing categorical custody integration target including both new canonical-channel cases, focused strict Composite test lint and strict AI library/channel-test lint with the numerical operation-owner feature. No Language dependency is needed for this check.

## First supported trial and fixture correction

The integration owner ran `cargo +stable xtask check canonical-export` at exact source `06e36ea8001db55264208fc41ac22d18cf9a7427`. The command exited **1**: all six generic export tests and twelve existing categorical custody tests passed, but both new AI canonical cases failed during fixture Plan construction because the input Fore value contract did not match the selected Back. Strict steps were not reached. The original raw-log SHA-256 is `21219c7b64eb69c56c060f7d0d722dd7ffbe53ffde9cbea6cacbe9aab73137ce`; the integration owner retains that failed log and its source-bound trial receipt. No failed case is counted as passing.

The correction changes only the AI test fixture: its Source now declares explicit input/output byte bounds computed from the actual prepared structured types. Additional assertions compare each authored Fore contract with the corresponding selected Back contract before planning. Named type aliases establish Kind identity; they do not supply those omitted explicit bounds. This follows the held canonical helper's approach without weakening the planner, adapter, runtime or contracts. The fixture-correction successor remains unrun; the earlier `06e36ea8` trial does not prove it. Untested predecessor `1f0030152b4a2f0eafe066c4267ef4ec2b92d2c0` remains recoverable and is not positive execution evidence.

## Acceptance boundary

Formatting and `git diff --check` were performed after the fixture correction. A successful rerun of the complete supported entrance and its strict steps is still required from the integration owner's cache-owning path. Historical held Language-model evidence is not relabelled as proof of this generic recovery. #6062's dependency status, required current-base CI, exact-head acceptance, reviewed merge and the parent's shared Integration/publication batch remain delivery gates; this preparation does not close #6063.
