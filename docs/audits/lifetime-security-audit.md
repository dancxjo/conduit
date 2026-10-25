# Lifetime and retired-identity security audit (#6409)

Audited development baseline: `31f2e04c3cf964e248dd72a15cc1a246fd004570`.
The public capability-limit tests were added and executed at
`1f6d33ca1f6bbe582a774fb2316c996926272cce`; this document is a
bounded follow-up, with separate actual native model execution at
`465af45add0195e4481e56fe90ed46cf74872d91` recorded below. It is not an
exhaustive counter inventory or full #6409 acceptance. No physical experiment
was performed for this document.

## Owner, counter and retirement table

| Owner / protection class | Scope and lifetime | Maximum issuance / source of uniqueness | Exhaustion | Revocation and late-delivery check |
| --- | --- | --- | --- | --- |
| [HTTP client/guard](../../targets/conduitos/src/http.rs), [request owner](../../targets/conduitos/src/http/request.rs); trusted synchronous Rust adapter | One fixed-storage client and exclusive `HttpRequestGuard` borrow. Transaction identity is descriptive, not possession. | One pending request; no detached ticket or reusable generation. Successful requests can continue sequentially while storage is available. Affine borrow, not numeric uniqueness, fences owner reuse. `signs:u16` saturates and is diagnostic. | Pending latch refuses Pressure; forgotten guard leaves it latched. Fixed request/response/typed-info limits refuse before publishing invalid output. | Consuming exchange/cancel/Drop retires the borrow, clears latch and scrubs private bytes. Exchange attempts the synchronous provider once. Failure preserves prior published output; no claim of rollback of a partially completed network effect or asynchronous cancellation. |
| [Kernel capability table](../../targets/conduitos/src/protection_domain.rs); trusted Root enforcement | One table, exactly eight permanently occupied slots, exact domain/Host/Boot/Plan/Play/Base/resource/operation/authority scope; one in-flight operation per handle. | At most eight successful issues per table; handle seals secret, domain, slot, issuance and full scope. Each handle admits at most `maximum_operations:u32` starts. | The apparently wrapping issuance cannot wrap within eight occupied slots. Operation sequence can wrap only after the last allowed start; the completed/in-flight gate prevents another grant. Completion cannot exceed the admitted u32 count. | Revocation is absorbing: revoked entries cannot be reused, and no operation frees a slot. Complete checks occupied/revoked, table generation, exact active sequence and in-flight=1. Repeated handle revocation may wrap its generation but never clears revoked. `authorize_current` also compares the entire current scope. **New-table secret/Boot/domain provenance remains a separate caller obligation.** |
| [Native service identity](../../targets/conduitos/src/arch/x86_64/service_domain.rs); Root admission for a protected x86_64 service | Root-process static `NEXT_INSTANCE:AtomicU64`, subordinate to exact installed service/Boot ownership. | Starts at one; checked `fetch_update` allocates distinct instances within that Root lifetime, at most u64::MAX−1. Failed admission may consume an ID; it does not refund it. | Checked addition refuses rather than wrapping. No forced Atomic maximum test is claimed here. | Actual instance plus envelope sequence fences service replies. Terminal quarantine retires the service. Local Atomic uniqueness does not establish global Boot uniqueness, cross-architecture containment, physical/DMA quiescence or correct routing by an external owner. |
| [Timer nucleus](../../targets/conduitos/src/timer_nucleus.rs); cooperative logical helper | One pending token plus one cancelled observation. Token is caller Play generation u32 plus request generation u32. | At most `request_capacity:u16` arms per epoch, so request wrap is unreachable within one admitted epoch. At this audited baseline, replan accepts repeated/older Play generations and resets the request counter. | Arm refuses AlreadyArmed/CapacityExhausted. **Baseline defect:** `new(7,1) → arm A → replan(7,1) → arm B` recreates A's token, so A can settle B. | Exact pending/cancelled comparisons alone do not fence repeated epoch identity. The prepared strictly-increasing-generation correction is not in this baseline; see the separate preparation record below. Even that correction does not authenticate a newly constructed timer owner or reacquire physical authority. |
| [Fixed value store](../../architecture/kernel/src/fixed_value_store.rs), [hosted store](../../architecture/kernel/src/hosted.rs); kernel-owned storage | Admitted finite slots/bytes; ValueRef names slot, u16 generation and byte length, with retained reference ownership. | At most 65,535 generations per slot. Final-generation live values remain valid until final release/clear; other reusable slots retain their normal selection order. | A free slot whose generation cannot advance is retired, not wrapped. When retirement prevents an otherwise fitting allocation, GenerationExhausted is explicit. | Clear preserves retirement and generation, so old same-length references cannot revive. Existing public full-domain and remote-ingress regressions are recorded in [value-generation-retirement.md](value-generation-retirement.md); not rerun here. This is storage custody, not physical/Host authority. |
| [Reusable value pool](../../architecture/composite/src/reusable_value_pool.rs), [controller](../../architecture/composite/src/bounded_activation/reusable.rs), [scan controller](../../architecture/composite/src/bounded_scan_activation/reuse_pool.rs); cooperative move ownership | One private child in Ready/Leased/Drained; prepared bindings/sign storage tied to parent Play, activation and invocation. | Nonzero u16 maximum, finite preprepared invocation vector. [Rearm](../../architecture/composite/src/kernel_executor/reusable_value.rs) requires exact definition Arc, parent/activation and checked next invocation. | Take refuses unavailable/wrong lifecycle; advance consumes one prepared binding. Failed rearm retains a drained owner and does not cold-retry. | Controller call sites move the active child back exactly once by source inspection. `retire` has a debug assertion, not release enforcement or isolation. No reachable public double-retire was demonstrated; forced public controller rearm-failure/panic/disposal proof remains open. |
| [Std worker](../../targets/std/src/kernel_preparation/worker.rs), [serve/retirement](../../targets/std/src/kernel_preparation/worker/run.rs); cooperative OS thread | Private Control Arc, one command/reply slot, one optional installed prepared kernel and owned JoinHandle. Install/Start are once-only. | Control/JoinHandle ownership replaces a reusable numeric worker ID. Prepared transfer/stack and mailbox capacities are explicit; stopping/exited refuses submission. | Unread command/reply gives Busy; no hidden retry or second kernel installation. | Serving and cancellation are separately caught for panic evidence. Shutdown takes JoinHandle once, joins and retains kernel/failure/cancellation custody; Drop attempts shutdown. Panic/poison reporting cannot hard-terminate arbitrary uncooperative code. Public effect callback/Host Call lease/Watch owners need their own replay proof. |
| [Protected runtime broker](../../targets/conduitos/src/protected_runtime.rs); protected guest plus trusted Root | Actual service instance, checked u64 request sequence, fixed per-node pending calls, one input and staged output custody. | Request starts one; checked increment before dispatch. Exact native response envelope and current pending node/request constrain publication. | Counter exhaustion returns DecisionLimitExceeded before dispatch. Output must match staged ValueRef and admitted Host Call byte bounds. | Error/cancel/abort quarantines domain, clears pending/ledger/references and zeros input/staged bytes. Complete requires exact pending request before guest ACK. Clearing software custody is not proof of external Host Call cancellation, native fault-to-fresh-B replay or DMA quiescence. |
| [Root input-loan/ingress ledger](../../targets/conduitos/src/protected_runtime/ledger.rs); trusted Root custody, not a scheduler or bearer | Fixed admitted cord/slot arrays and exact loan bytes/optional Root ValueRef. Per-cord u64 ingress sequence starts zero; u16 queue heads advance modulo admitted capacity, and len never exceeds that capacity. | Exact endpoint+cord and expected sequence; declared item/byte limits and fixed storage. Numeric node/request/value identities are local to their selected owner. | u64::MAX refuses before publication through checked_add. Pressure creates no loan and consumes no sequence; accepted ingress increments only after preflight. Public maximum exhaustion has not been forced. | Accept consumes one exact Root loan; fabricated/duplicate bytes or references refuse. Clear removes loans **and route grants**, not just sequences. Root [Patchbay owner gate](../../targets/conduitos/src/native_workset/play/patchbay.rs) checks current Body/Plan/Play and liveness before publication/ingress. Calling fresh B with A's matching local numbers alone is not a cross-owner authority test. |
| [Native model supervisor](../../targets/std/src/native_model_backing/supervisor.rs), [worker](../../targets/std/src/native_model_backing/worker.rs); selected Linux subprocess profile | One bootstrap, one guarded-stack thread and frozen process; **multiple inference calls and ordinary Plays may reuse it** under retained exact admission. | Global checked AtomicU64 transport ID; per-transport u32 request sequence bounded by `maximum_calls:u16`. Current request identity is exact transport+sequence, not caller transaction equality. | `fetch_update(checked_add)` refuses global transport exhaustion; per-session call ceiling prevents u32 wrap. No forced Atomic maximum or public process-owner exhaustion execution is claimed here. | Supervisor checks current/failed, validates sequence/result/ACK, and keeps Ready private until ACK. Unload marks failed/revokes exchange before pipe close and kill/wait. Worker loops maximum_calls then `_exit`; FROZEN/CONTEXT are process-local, not single-inference evidence. Kill/wait failure paths and A→B late-owner matrix remain separate proof obligations. |

## Executed proof versus source inventory

| Surface | Evidence classification | Remaining boundary |
| --- | --- | --- |
| Domain table/operation limits | At `1f6d33ca1`, a small external Cargo harness compiled the **unmodified production** table and helper files through symlinks, and the exact repository [public lifecycle test file](../../targets/conduitos/tests/protection_domain_lifecycle.rs). Eight tests passed: two new public cases, two existing public cases and four existing production tests. Strict harness Clippy, focused rustfmt and patch checks passed. | This is actual-source CPU/public-owner proof, not the complete `conduitos` package, protected guest, native fault or physical execution. The initial harness module-path compile refusal was corrected in harness layout only; raw negative/positive logs and source hashes remain retained. No u32 counter mutation hook or mirrored implementation was used. |
| Exact new cases | Full eight-slot table refuses a ninth issue; revocation never recycles slots and rejects old handles/leases. Two-operation admission refuses excess/in-flight work; replaying the first completion cannot settle the second lease; the second completes exactly once and sibling authority remains usable. | It proves admitted finite limits, not billions of operations, global issuer uniqueness or newly constructed tables using repeated secrets/scopes. |
| HTTP guard | Public cases exist in [http_ticket_lifetime.rs](../../targets/conduitos/tests/http_ticket_lifetime.rs); compile-fail borrow/consumption cases exist in request.rs. Source includes repeated transaction/bytes, provider/refusal, sibling and forgotten-guard controls. | Not rerun by this audit. Do not reuse the historical detached-ticket ABA diagnosis against this guard implementation or infer asynchronous/network rollback proof. |
| Root loans and authority route | Actual numeric-kernel/Root-ledger cases exist in [ledger_tests.rs](../../targets/conduitos/src/protected_runtime/ledger_tests.rs); current public owner checks exist in native_workset/play and [product journey workset tests](../../targets/conduitos/src/product_journey/workset_tests.rs). | Test presence is not a new passing result. Hosted loan checks do not instantiate a protected Broker fault, and local numbers are not owner routing capabilities. |
| Composite reuse | Existing controller/rearm tests were inspected. | Not rerun here. No new rearm-fault, panic or disposal proof. |
| Native model process | At `465af45ad`, all three ignored [owning lifecycle cases](https://github.com/dancxjo/conduit/blob/465af45add0195e4481e56fe90ed46cf74872d91/targets/std/tests/model_infer_native_lifecycle.rs) passed against explicitly pinned installed ORT on Linux x86_64 GNU. Actual Source/Plan/Play selects the native worker. Two ordinary Plays retain the same worker PID/starttime, mappings and admission; host drop reaps it. Ordinary Stop cancels with nine request bytes queued, emits no output and reaps the child. SIGKILL provider loss returns the existing String `BackFailed`/`HostCallFailed` detail 71, emits no output and spawns no retry child. | SIGSTOP establishes pending cancellation, not interruption during native CPU computation. These cases do not force panic, kill/wait failure, late-completion races, fresh-owner A→B replay, Atomic exhaustion or cross-artifact/profile reuse. They establish no hostile confinement, physical authority, native performance or CI/release acceptance. |

The CPU harness/source/log hashes are retained in this task's
`work/domain-capability-limits-handoff.json`; execution Source is `1f6d33ca1`.
Its supported owning-package entrance remains
`cargo +stable test --locked -p conduitos --test protection_domain_lifecycle`.
That package entrance, Candidate and merged-source Integration were not run by
this lane. The document does not substitute the external harness for those gates.

The model run used the internal command `cargo +stable test --locked -p
conduit-std-host --features native-model-backing --test
model_infer_native_lifecycle -- --ignored --test-threads=1 --nocapture`, with
explicit `CONDUIT_ORT_LIBRARY`. The existing `ModelNativeInference` xtask suite
registers this test as `model.native-lifecycle`, including `--ignored` and a
single test thread. This receipt covers the direct Cargo step, not execution of
the complete supported suite. ORT SHA-256 was
`9c8f6112e9dcf09ed4e621a1d0a7b6ba49575c7d09f27a652d24a22c8564b13c`.
The exact passing test ELF, worker ELF, runtime and raw log are sealed outside
Git in this task's ignored
`work/constructed-face-reuse/target/model-lifecycle-proof/465af45ad/`, with
`receipt.json` and verified `SHA256SUMS`. Test ELF SHA-256 is
`1a4df7513480340ef8cf69c6ad6d4f08d3503679b16b2937a514b8f76d34588c`;
worker ELF SHA-256 is
`de3edfc79f0adaaca75927d4051bee8281f792eb020db677415a5472b07c5c5c`.
The initial temporary-target attempt exhausted the user's temporary quota
before execution; its negative log remains retained separately. The subsequent
disk-target run passed three cases in 21.02 seconds. Its inactive compiler cache
was cleaned only after sealing the passing executables and checking process
ownership. These are local receipts at their stated source, not merged-baseline
or stable-release claims.

## Separate preparations, not merged baseline behavior

These independent recovery branches are not in audited baseline `31f2e04c3`.
Their local receipts belong to their stated source; this document does not
relabel them as current dev, Candidate or Integration acceptance.

- Timer correction: `work/6409-timer-lifetime-20261025` at
  `ef5a4476a70a078064d13f0c16a7ab05de57be8c`. Its
  [source-pinned audit](https://github.com/dancxjo/conduit/blob/ef5a4476a70a078064d13f0c16a7ab05de57be8c/docs/audits/timer-nucleus-lifetime.md)
  reports six actual-module tests in debug/optimized modes plus a compile-fail
  ownership case. Strictly newer generations and move-only ownership fence
  **same-owner** replay; fresh-owner identity remains an integration obligation.
- Std retirement preparation: `work/6409-worker-retirement-20261025` at
  `5e128c3e9ef0ed2054467c3eb3643bfb4619064b`. Its
  [audit](https://github.com/dancxjo/conduit/blob/5e128c3e9ef0ed2054467c3eb3643bfb4619064b/docs/audits/std-worker-retirement.md)
  reports two focused cases and strict Clippy through actual StdHost/service and
  OS threads, including panic plus failed cancellation, exactly-once disposal,
  old Plan/delivery refusal and fresh B/sibling controls. It does not cover the
  full effect callback/lease/Watch matrix, arbitrary hard termination or a
  surviving semantic Body replan.
- Browser lifetime preparation: `work/6409-browser-lifetime-20261025` at
  `24197e5ce6aa3ecac64a89c8468005e728c9c8f8`. Its
  [audit](https://github.com/dancxjo/conduit/blob/24197e5ce6aa3ecac64a89c8468005e728c9c8f8/docs/audits/browser-body-host-lifetime.md)
  reports actual local WASM/Chromium A-close→B, one worker/zero retries, old
  effect tuple refusal and post-promise-settlement callback/input controls.
  This is page-owned WASM/BodyHost proof, not execution WebWorker termination,
  debugger Watch, independent Host isolation or physical authority.

- Ordinary StdHost Host Call retirement: `work/6409-std-hostcall-retirement-20261025`
  at `63d60583ba3c4759a1c593192e2a486cf192ac35`. The actual owning-package
  [Vision conformance fixture](https://github.com/dancxjo/conduit/blob/63d60583ba3c4759a1c593192e2a486cf192ac35/targets/std/src/installed_std_tests/remote_vision_conformance.rs)
  passed both focused tests and strict library/test Clippy. It starts a real
  pending request with admitted input and staged value, cancels owner A, and
  observes `HostCallCompletionRejected` on that retired owner. The same
  persistent StdHost then admits B with the same Host/Boot/Plan and a fresh
  Play sequence (0 to 1); B's own request completes exactly once and drains.
  An independent source-owner sibling remains live. Mutable admitted-owner
  custody is trusted Root authority; request/value numbers are owner-local
  descriptors. This does **not** prove arbitrary cross-owner numeric-tuple
  refusal, asynchronous callback/Watch revocation, hostile confinement or
  physical authority. The exact passing executable and logs are retained
  losslessly in the task's `work/std-hostcall-retirement-proof/` custody.
- Browser input sequence exhaustion: `work/6409-browser-input-exhaustion-20261025`
  at `d227f2a3d8f85b7bcaffb06f87dc73b67ed5fffd`. The existing
  [input adapter](https://github.com/dancxjo/conduit/blob/d227f2a3d8f85b7bcaffb06f87dc73b67ed5fffd/targets/browser/host/assets/browser-human-input.mjs)
  now retires pointer sequences at their u32 boundary and button sequences at
  the safe-Number boundary before their u64 encoder. The final representable
  value is emitted once; further input refuses rather than truncating or
  reusing an imprecise Number. Two pure-policy boundary tests and all thirteen
  existing Chromium DOM input cases passed (one worker, zero retries).
  Boundary tests execute the actual advancement policy, not billions of DOM
  events or a forced full WASM roundtrip. This preparation is not in the
  audited baseline; it grants no new owner/input authority and does not
  re-execute or relabel the earlier WASM lifetime proof.

A separate source review found unchecked u32 Tour timer/manifestation completion
**reporting** counts. Pending effect correlation uses Play, placement and request,
not those counts. No authority bypass or universal lifetime bound was established.
A future checked-arithmetic refusal could precede kernel completion/storage, but
cannot precede a platform timer or DOM effect already performed before its
completion arrives. Retain that boundary instead of claiming rollback or a
pre-platform-effect guarantee from a reporting-counter correction.

## Remaining #6409 acceptance

A complete current owner/counter table and joined negative matrix remain
required. Prioritize actual external owner fences over speculative counter
rewrites: no new domain-table or model Atomic defect was demonstrated here.
The timer baseline defect has its separate unmerged correction above.

Still required: actual native protected broker/Root owner A→fault/cancel/retire→B;
all exposed old completion/callback/Host Call lease/Watch/input/staged-output
classes; poisoned and failed cleanup, provider loss, worker exit and late-response
races; exactly-once disposal where promised; sibling non-effect; and surviving
semantic Body with freshly checked physical placement/lease/Base authority.
Where an API does not expose a class, retain unsupported/unverified truth rather
than inventing a callback or treating a numeric tuple as possession.

Permanent existing security/lifecycle entrance registration, exact-source
negative receipts and relevant Candidate/Integration remain gates. This audit
proves no full #6409 completion, physical/DMA authority continuity, general
hostile-code containment or native preemption/continuation repair. Keep
#6311/#6317/#6184 mechanism ownership separate.
