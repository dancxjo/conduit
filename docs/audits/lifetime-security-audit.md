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
| HTTP guard | At `8b75b1b9987cc62de9de437596b96a5824b2af55`, the actual owning-package [http_ticket_lifetime.rs](../../targets/conduitos/tests/http_ticket_lifetime.rs) case passed with `hosted-tools,native-compositor,native-http-client`. It exercises repeated transaction/bytes, provider/refusal, sibling and forgotten-guard controls. | The earlier Candidate admission target ran zero cases because its feature was disabled. This preparation enables the feature in the existing host-protocol gate. Ownership doctest presence is not a new doctest execution; synchronous refusal is not asynchronous/network rollback proof. |
| Root loans and authority route | Actual numeric-kernel/Root-ledger cases exist in [ledger_tests.rs](../../targets/conduitos/src/protected_runtime/ledger_tests.rs); current public owner checks exist in native_workset/play and [product journey workset tests](../../targets/conduitos/src/product_journey/workset_tests.rs). | Test presence is not a new passing result. Hosted loan checks do not instantiate a protected Broker fault, and local numbers are not owner routing capabilities. |
| Composite reuse | Six existing scheduler rearm cases passed in Candidate40196598054 admission: active/cancelled refusal, prior failure, missing evidence, stale input, lost resident pin and exhausted sequence identity. Selected pool/controller/scheduler sources were byte-identical to dev5b483a91. | Existing finite owner/rearm proof, not a new panic-in-rearm, destructor or native external-resource disposal witness. The private moved-child source audit does not turn its debug assertion into isolation enforcement. |
| Native model process | At `465af45ad`, all three ignored [owning lifecycle cases](https://github.com/dancxjo/conduit/blob/465af45add0195e4481e56fe90ed46cf74872d91/targets/std/tests/model_infer_native_lifecycle.rs) passed against explicitly pinned installed ORT on Linux x86_64 GNU. Actual Source/Plan/Play selects the native worker. Two ordinary Plays retain the same worker PID/starttime, mappings and admission; host drop reaps it. Ordinary Stop cancels with nine request bytes queued, emits no output and reaps the child. SIGKILL provider loss returns the existing String `BackFailed`/`HostCallFailed` detail 71, emits no output and spawns no retry child. | SIGSTOP establishes pending cancellation, not interruption during native CPU computation. These cases do not force panic, kill/wait failure, late-completion races, fresh-owner A→B replay, Atomic exhaustion or cross-artifact/profile reuse. They establish no hostile confinement, physical authority, native performance or CI/release acceptance. |

The CPU harness/source/log hashes are retained in this task's
`work/domain-capability-limits-handoff.json`; execution Source is `1f6d33ca1`.
Its supported owning-package entrance remains
`cargo +stable test --locked -p conduitos --test protection_domain_lifecycle`.
The owning-package gap was subsequently closed locally at
`8b75b1b9987cc62de9de437596b96a5824b2af55`: all four public capability
lifecycle cases and the one affine HTTP case passed in the actual package,
using `hosted-tools,native-compositor,native-http-client`. Focused strict
Clippy for both integration targets and workspace formatting also passed. The two new capability
cases are included in those four, rather than relabeled external-harness tests.
Exact executable archives and logs are retained outside Git. This is the focused
owning-package entrance, not all package tests, native guest execution, Candidate
or merged-source Integration acceptance.

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

- Ordinary StdHost receiver retirement and retained Body replan:
  [PR #6491](https://github.com/dancxjo/conduit/pull/6491), merged into dev at
  `5b483a91a630d39dbd692c97c74b1f9758a51ff4` after Candidate
  [40195009584](https://github.com/dancxjo/conduit/actions/runs/40195009584)
  passed at `3f4864c18f2978e57fd9dcb08309f841c17d1a3a`. Its actual admission
  log confirms both owning regressions and ownership doctests. Local validation at
  `3daddea9ca346d7499540a4f08b44b83a8fe868c` passed all ten Todo integration
  cases, two Vision cases, two receiver-ownership doctests and strict
  library/Todo Clippy. The subsequent conflict-free merge of dev `100ae115`
  has not been relabeled as that execution. The
  [scoped audit](https://github.com/dancxjo/conduit/blob/3daddea9ca346d7499540a4f08b44b83a8fe868c/docs/audits/std-hostcall-retirement.md)
  records exact executable/log custody outside Git.
  Vision cancels A with pending input and staged output, rejects completion on
  A, consumes its receiver, then admits B on the same Host/Boot. B reaches the
  identical receiver-local descriptor and input, remains pending without output
  or sibling changes, then completes once through the real synchronous provider.
  No detached A completion is delivered after B; local numbers are descriptors,
  not bearer authority. The source sibling is released and re-admitted, not
  continuously admitted across replacement. Ownership doctests prove consuming
  receiver use, not runtime isolation.
  The Todo case retains one actual Body and Wake across a checked Host-offer
  transition: old Plan refuses before start/output/storage changes, old grant
  refuses `AuthorityGrantMissing`, and a fresh checked Plan selects new
  placement/capability while retaining semantic workset. Each real start runs
  once. Typed Fore bytes explicitly carry Todo state between episodes; no
  worker-global state continuity is inferred. This profile exposes no external
  bearer lease or separate physical Base handle. Async callback/Watch revocation,
  hostile confinement and physical authority remain outside these cases.
- Browser finite input and terminal owner correction:
  [PR #6492](https://github.com/dancxjo/conduit/pull/6492), merged normally into dev
  at `d3356f28a341a02f212e1030fc456a51076f125e` after Candidate
  [40196598054](https://github.com/dancxjo/conduit/actions/runs/40196598054)
  passed at `53b0fd621efeff8b22f2757ba8fc0df2821ad092`. The product log
  records all 276 Chromium tests passing, including all thirteen human-input
  cases and the terminal PageLost regression. Pointer sequences retire at the
  u32 boundary and buttons at the exact safe-Number boundary before u64 encoding;
  the last representable value emits once, then issuance refuses. Capacity is
  checked before routing/pointer capture, failed button streams cannot route,
  and terminal handlers refuse. Terminal causes except recoverable FocusLost
  are absorbing: blur/focus cannot downgrade PageLost and revive an owner.
  At that exact source, two actual advancement-policy boundary cases and all
  thirteen pinned Chromium DOM cases passed with one worker and zero retries.
  The real DOM PageLost regression checks no further routing/pointer delivery
  and refusal after blur/focus; its pre-fix negative is retained outside Git.
  The supported browser check suite registers the policy cases. These tests do
  not inject a maximum into a live owner, run billions of events or prove the
  counter boundary through WASM. They neither grant new authority nor rerun the
  earlier WASM lifetime proof. The two policy-boundary cases have local execution
  and permanent named xtask registration; the 276-case Chromium CI result must
  not be substituted for forced live-owner maximum or WASM counter-boundary proof.

A separate source review found unchecked u32 Tour timer/manifestation completion
**reporting** counts. Pending effect correlation uses Play, placement and request,
not those counts. No authority bypass or universal lifetime bound was established.
A future checked-arithmetic refusal could precede kernel completion/storage, but
cannot precede a platform timer or DOM effect already performed before its
completion arrives. Retain that boundary instead of claiming rollback or a
pre-platform-effect guarantee from a reporting-counter correction.

## Scoped native protected-fault execution

The existing supported `cargo +stable xtask make conduitos ordinary-domain-proof`
completed successfully at source `b0bc723e2348b62befbfd2259b2c123e4338916c`.
This is a freestanding x86_64 ring-3 QEMU architecture proof appliance, not the
normal product or physical execution. Its actual Root ELF SHA-256 is
`bebf7d26c256dc2bbe1eebc4d4d9850d0d01dd35f1f74207887aa83613aedc9d`.
The matching build record, complete newline records and sealed executable custody
remain outside Git under this task's `work/p0-publication/target/`, in
`fault-pc-proof/` and
`fault-pc-executable-custody/native-proof-receipt.json`.

- `compositor-hostile-broker-fixture@1` exercises actual protected CPU fault
  and loop termination through production CompositorService: quarantine retires
  A, fresh production guest B has a greater instance while A is still alive,
  A request/upload refuses Terminated without added cost, and dropping A does
  not retire B. B remains usable; Root and unrelated TextDomain sentinels remain
  unchanged. Envelope/shape refusals also pass. Sentinel checks are not an
  independent direct-memory-attack or general isolation proof.
- `compositor-copy-recovery-fixture@4` separately exercises Root copy failure,
  released staging and activation/remove storage, explicit fresh ownership,
  stale-client refusal and stale-drop non-effect while old clients remain alive.
  It retains an explicit current display grant. This is not a guest CPU fault;
  the two cases do not establish one joined CPU-fault-plus-staged-display witness.
- Service-memory cases refuse enter/exchange/output/transfer-window after fault,
  charge teardown, sample zero restart boundaries, and retain sibling sentinel
  and positive-entry controls. Serial capability gates exercise exact scope,
  replay/exhaustion, revocation and provider replacement. One invalid-next-gate
  case deliberately permits an earlier valid serial effect; there is no blanket
  rollback claim.

The inspected protected-runtime and compositor hostile/recovery owning files are
unchanged from this execution source through P0 source `195b512c01494fbc17c4423b2e377c3e7f8af9b1`;
this comparison is not a whole-head rerun. Service-memory/native entry code
changed between dev `100ae115` and that P0 source, so the receipt is not dev100
execution. The records explicitly deny semantic Show acknowledgement, physical
scanout, driver isolation and DMA isolation where applicable.

## Remaining #6409 acceptance

Reconcile the finite audited-owner table and applicable negative matrix with
merged code and exact-source receipts. No new domain-table or model Atomic
defect was demonstrated here. Timer ABA and browser finite-input/terminal-owner
corrections remain separate unmerged preparations; the Std receiver/Body cases
are positive scoped proof, not newly demonstrated production defects.

Generic protected CPU fault→quarantine→fresh B is not wholly unproved: the
native cases above cover that class within their stated owner/profile. A joined
canonical Runtime Broker case with a real pending Host Call, staged Root ValueRef,
controlled native fault and old completion while B is pending remains unverified
**if that specific join is claimed**. Ledger tests alone do not execute it, and
local node/request/value tuples do not authorize cross-owner delivery. This is
not evidence of a new defect or a requirement to repeat every fault on every
owner. No detached callback/Watch, DMA or network-grant obligation is invented
for profiles that expose none.

Retain explicit limits for applicable poison/failed cleanup, late-response races,
exactly-once disposal and sibling effects. The Std retained-Body case supplies a
checked replan witness at its stated source, not physical authority continuity.
Permanent existing security/lifecycle entrances, exact-source negative receipts
and relevant merged-source validation remain acceptance work. This audit marks
no #6409 checkbox complete and proves no full issue completion, physical/DMA
authority continuity, general hostile-code containment or native
preemption/continuation repair. Keep #6311/#6317/#6184 mechanism ownership separate.
