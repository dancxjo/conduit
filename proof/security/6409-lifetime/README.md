# Retired identities: finite source audit for #6409

Audit Source: `79469ccd40486921a215b6f73dde967d874c0bbc` (captured dev).
This preparation changes documentation only. No tests, builds, native execution,
CI, persistent-Host A→retire→B proof, memory-safety proof or issue acceptance is
claimed. `sources.json` binds the inspected files to their exact source hashes.
The table covers the issue's named seams and directly related ownership gates;
it is not an exhaustive inventory of every repository counter.

Canon: exact Plan/Play and Base authority remain distinct; serializable identity
is not possession. Finite admitted state and typed refusal remain mandatory.
`catch_unwind`, cooperative cancellation, debug assertions and elapsed time do
not establish hostile-code containment. Existing #6311/#6317/#6184 native
preemption owners, Application/Face, Quantity #6393, GUI #6351 and #6252 remain
outside this preparation's edit ownership.

## Identity and retirement table

| Owner / protection class | Identity, lifetime and issuance bound | Increment / exhaustion / reuse decision | Revocation and late-delivery gate | Finding / proof boundary |
|---|---|---|---|---|
| `NativeHttpClient`, trusted in-process native adapter | `HttpTicket { generation:u32, transaction:u64 }`; one pending ticket per client; transaction is supplied in request; every client starts generation 1 | `begin` copies generation; successful `complete` clears pending without advancing it. `cancel` wraps generation and skips zero. No total request/cancel bound enforced. `signs:u16` saturates and is diagnostic, not lifetime authority | `request_bytes` checks pending equality; `complete` also checks current generation. Cancel clears pending before advancing | **Immediate cross-client identity alias and successful-reuse alias in source**, in addition to eventual cancellation wrap. No instance/Boot/Play binding in ticket. Exact reachability of stale network completion at installed effect boundary is still unproved. Refs: [http.rs:39](../../../targets/conduitos/src/http.rs#L39), [81–180](../../../targets/conduitos/src/http.rs#L81) |
| `ReusableValuePool`, private composite owner | Exactly one child in Ready / Leased / Drained; maximum `u16` invocations admitted once; pending bindings/evidence reserved before Play | `take` moves ownership, refuses unless bound/available/Ready; `retire` only debug-asserts Leased. `advance` consumes next prepared binding, keeps host Drained on rearm failure; no cold retry/fallback | Public `KernelCompositeHost::rearm_value` requires started, not cancelled, one child, no links/pending Host Calls, same definition Arc, exact parent/activation, and checked next invocation | Debug assertion alone is not enforcement. Public mutation of pool is unavailable. No demonstrated duplicate-retire path: controller takes its owned child; failed prepared ingress cancels/retires and faults owner. Need public controller panic/failure/rearm-refusal proof before alleging exploit. Refs: [pool:27–116](../../../architecture/composite/src/reusable_value_pool.rs#L27), [rearm:114](../../../architecture/composite/src/kernel_executor/reusable_value.rs#L114), [controller:117–140](../../../architecture/composite/src/bounded_activation/reusable.rs#L117) |
| Composite invocation / semantic continuity | Parent ActivePlayId + activation + internal Plan/fragment + invocation; private prepared binding holds definition Arc | Precomputed indices 0..maximum-1; `invocation.checked_add(1)` enforces succession. Evidence moves to immutable prior record; finite `u16` index is not wrapping reuse | Definition pointer equality prevents equal-valued foreign owner substitution; old active Play map replaced, mandatory signs moved; no authority or new Boot follows from reservation alone | Existing reusable-child unit fixture covers ordinary reuse/no allocation, not all public controller fault/reentrant paths or persistent hostile effects. Refs: [reusable_value.rs:9–168](../../../architecture/composite/src/kernel_executor/reusable_value.rs#L9) |
| Std region worker, trusted Rust thread | One private Control Arc, one command/reply slot, one installed `PreparedRegionKernel`; Install and Start each once | New Control/thread per worker; stopping/exited reject submission, unread command/reply gives Busy; no shared reusable numeric worker generation | `serve` catches unwind, attempts cancellation, marks exited, retains kernel/failure/cancellation; shutdown sets stopping, takes JoinHandle once, joins; Drop also attempts cleanup | Panic/poison are typed outcomes, **not hard termination**. `join` can block on an uncooperative Back. Existing panic retirement fixture does not establish persistent Host A→B rejection of all late callbacks/leases/observations or failed-cleanup exactly-once disposal. Refs: [worker:79–213](../../../targets/std/src/kernel_preparation/worker.rs#L79), [290–315](../../../targets/std/src/kernel_preparation/worker.rs#L290), [run:3–60](../../../targets/std/src/kernel_preparation/worker/run.rs#L3) |
| ConduitOS protected runtime broker, protected guest plus trusted Root | Instance from actual service domain; request `u64` starts 1, checked increment before dispatch; pending Host Call bounded per node | Exhaustion returns DecisionLimitExceeded before sending; exact response envelope and bounded ledger gate normal execution | Quarantine drops domain, clears pending/ledger/input/staged and zeroes buffers; completion requires exact pending node/request before publication | Source cleanup is not executed fault/reuse proof. Need actual old-instance completion/input/staged-output replay after B admission and sibling sentinel. Instance uniqueness belongs native owner and is not reimplemented here. Refs: [protected_runtime.rs:87](../../../targets/conduitos/src/protected_runtime.rs#L87), [128–140](../../../targets/conduitos/src/protected_runtime.rs#L128), [162–191](../../../targets/conduitos/src/protected_runtime.rs#L162), [340–387](../../../targets/conduitos/src/protected_runtime.rs#L340) |
| Native capability table, trusted Root enforcement | Fixed 8 entries; issue chooses never-occupied slot, no slot-free operation; exact domain/Boot/Plan/Play/Base/resource/operation scope; one in-flight operation | Issuance wraps in code but at most 8 successful issues per table. Per-handle next sequence wraps after issuing u32::MAX, but maximum_operations:u32 and completed+inflight gate preclude issuing repeated sequence. Revoke increments generation once per live entry, marks irrevocably revoked | `authorize_current` compares exact current scope, authorize checks claim/bounds/revoked; complete checks slot, generation, active sequence and in-flight=1 | **Locally bounded wrap, not automatically a defect.** Table reconstruction with repeated secret/domain/scope needs caller lifetime uniqueness audit; no global Boot uniqueness inferred. Refs: [protection_domain.rs:73–188](../../../targets/conduitos/src/protection_domain.rs#L73), [194–235](../../../targets/conduitos/src/protection_domain.rs#L194), [scope:245–261](../../../targets/conduitos/src/protection_domain.rs#L245) |
| Timer nucleus, public logical helper | Token=(caller play_generation:u32, request_generation:u32); request capacity u16 per epoch | At most 65,535 arms per construction/replan; wrapping request increment cannot wrap within epoch. `replan` resets counter and accepts any caller generation, including same/older | Pending/cancelled equality gates completion; replan clears both | Public `new(7,1)→arm(A)→replan(7,1)→arm(B)→complete(A)` aliases B in source. Caller uniqueness is unenforced. Only test call sites found in this captured tree; no installed-timer exploit asserted. Distinct from reserved native RTC/preemption. Refs: [timer_nucleus.rs:33–85](../../../targets/conduitos/src/timer_nucleus.rs#L33) |
| Native model worker supervisor, OS subprocess profile | Checked global AtomicU64 transport ID + u32 sequence; maximum_calls:u16 finite profile; worker process one thread/one bootstrap | Transport uses fetch_update checked_add (refuses exhaustion); sequence bounded by u16 maximum before increment; one pending request; terminal transport never renewed | Exact current request and failed flag; unload sets failed and revokes exchange before closing pipes, kill/reap. Worker fixed-stack context and FROZEN are single-shot process locals; each return calls `_exit` | Keep actual process-lifetime confinement; static mut alone is not a vulnerability. Kill error is ignored, wait outcome needs failure-path proof. Existing lifecycle/process tests are not all #6409 persistent-Host fault classes. Refs: [supervisor:128](../../../targets/std/src/native_model_backing/supervisor.rs#L128), [198–275](../../../targets/std/src/native_model_backing/supervisor.rs#L198), [worker:78–117](../../../targets/std/src/native_model_backing/worker.rs#L78), [188–236](../../../targets/std/src/native_model_backing/worker.rs#L188) |
| Kernel fixed/hosted value storage, trusted store | Per-slot u16 generation; live references include slot/generation/byte length | Final-generation slots retire on final release/clear; allocation skips them; GenerationExhausted if retirement prevents fit; clear preserves generations | Inspect/retain/release require exact live identity; accepted #5231 ancestry documents public full-domain exhaustion | Reuse the existing proof; this audit did not rerun it. Does not prove worker/callback/Host Call lifetime. Refs: [value retirement audit](../../../docs/audits/value-generation-retirement.md), [fixed store:127–192](../../../architecture/kernel/src/fixed_value_store.rs#L127) |

## Smallest high-value correction and proof sequence

1. **Own HTTP ticket lifetime separately first** (`targets/conduitos/src/http.rs`
   and its owning tests; no edits authorized by this documentation). Public API
   counterexample needs no counter injection: client A `begin(request,true)`
   retains ticket A, A `cancel()`, fresh client B `begin(the same request,true)`;
   B `request_bytes(ticket A)` matches in the current source. Assert stale
   refusal while ticket B remains current. This is a helper-level persistent
   harness with two client lifetimes, not the full installed Host proof.
2. Successful-reuse alias is also present in source: pending is cleared but
   generation unchanged. To execute it with an externally retained A ticket,
   preserve the actual provider response-fill path. The public `exchange` API
   hides its ticket and `complete` cannot populate the private response buffer;
   do not claim a pure-public single-client successful-ticket replay has already
   run. Existing unit provider fixture can establish the source defect, but that
   private access is a different proof class from installed completion replay.
3. A correction needs both per-owner and per-issued-request uniqueness, explicit
   checked exhaustion, and cancellation/fault retirement without identity reset.
   Advancing only on cancel or only making its increment checked is insufficient.
   Choose a finite profile-bound owner allocation with its existing Root/Host
   admission; do not introduce ambient authority or an unreviewed global reset.
   Validate same transaction/bytes, same receiver slot, repeated cancel, successful
   completion, provider loss and preserved B output. Use deterministic finite
   exhaustion through an explicit admitted profile/API, not mutating opaque
   generation fields. No new lifetime API is selected by this audit.
4. **Std persistent Host proof**: extend the existing region-worker control
   fixture at the actual public prepared-host boundary. Retain A completion,
   callback/Host Call lease, Watch/input observation and staged output; retire A
   through each supported fault class; admit B with fresh exact authority; replay
   each A item, assert typed refusal, B/sibling state unchanged and disposal count.
   Private test Back panic is useful fault stimulus, not proof of hostile thread
   termination. If a profile lacks a mechanism, record refusal/unsupported.
5. **ConduitOS mechanism**: coordinate native owner; use protected broker actual
   quarantine/replacement fixture, replay old instance/request at true boundary.
   Do not edit native continuation/RTC/Application ownership. Timer helper replay
   is a useful separate API negative, not a substitute for protected-domain proof.

## Existing evidence and missing acceptance

Inspected, not rerun: HTTP cancellation negative in
`targets/conduitos/src/http/tests.rs:139`; reusable no-allocation evidence in
`architecture/composite/src/kernel_executor/reusable_value/tests.rs:149`;
`targets/std/tests/region_worker_control.rs:68,215,256` covers normal mailbox,
panic retirement, and controller transfer; `targets/conduitos/tests/protection_domain_lifecycle.rs:41,85`
covers current authority dimensions/revocation; timer test at line111 uses a
*different* Play generation; native model supervisor/process fixtures retain
single-shot process confinement. None establishes the complete #6409 matrix.

The existing public `cargo +stable xtask check security-acceptance` dispatch is
in `tools/xtask/src/commands/check.rs:305–425`; it does not currently list the
region worker, HTTP ticket reuse, timer reset or reusable-pool tests. Permanent
registration in existing security/lifecycle entrances remains required; preserve
native isolation invocation and do not replace it with helper tests. Existing
`model-inference` native lifecycle suite is a separate relevant entrance.

Remaining full issue gates: executed public exhaustion/replay tests; std **and**
ConduitOS/browser persistent-Host A→retire→B; panic/poison, provider loss, exit,
protected fault, failed cleanup and cancel/late-response race; exactly-once
promised disposal; surviving semantic Body with freshly admitted placement,
lease/capability/authority; exact-source negative matrix and relevant CI. A table,
helper regression or smaller correction cannot close the full issue.
