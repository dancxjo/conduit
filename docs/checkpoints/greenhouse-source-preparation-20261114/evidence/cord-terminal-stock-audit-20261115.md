# Cord terminal payload stock — safety audit, 2026-11-15

**Verdict: the proposed topology-gated split has a sound semantic basis.** No counterexample was found if the compact profile counts **every logical Remote-source/Local-sink Cord, including local Fore**, prohibits every active AbnormalTerminal track, and preserves exact terminal bytes through reads, replay checks, cancellation and rearm. This is independent of the unsafe FIFO stock reduction previously reviewed.

Read-only audit at HEAD `816230877a05e1e9e3c21e8e2bb7f55d460ce9f7`; no builds or source edits. The other worker owns actual Source qualification. This report proves the storage-domain invariant from current code, not an implemented patch or measured saving.

## Storage-domain proof

Currently `CordState` contains `abnormal_terminal: Option<CanonicalValue>` on every Cord. CanonicalValue retains length plus the entire100-byte envelope. A search of production scheduler modules finds only two mutation domains after initial construction:

1. `scheduler.rs::close_remote_input_terminal`: validates the exact Cord index and `(Remote(source),Local(sink))` endpoints, matching endpoint identity, then writes terminal, producer_closed and producer_abnormal. Both ordinary and abnormal single close use this method. `remote_fanout.rs::close_remote_input_fanout_terminal` prevalidates each target and replay identity, then delegates each write to it.
2. `scheduler.rs::abnormally_terminate_outputs`: first scans the output's targets and counts tracks equal to AbnormalTerminal. It returns `Ok(false)` **before changing any Cord** when that count is zero. Only after a nonzero count does it close every sibling and store a full terminal on the non-AbnormalTerminal siblings.

Thus, if the entire actual active topology contains zero AbnormalTerminal tracks, domain2 cannot write terminal storage, even for spontaneous abnormal output, semantic failure or recovery/finalization outcomes. Those outcomes still return their original full terminal through the existing error path. Domain1 can write only logical remote-source/local-sink Cords. Initial storage is None and ordinary rearm restores None, completing the inductive storage-domain proof.

Do not narrow domain1 to `!spec.is_local_fore()`: local Fore intentionally shares the logical boundary queue mechanics while having no network Line authority. Its close APIs can also receive full abnormal terminals. Ignoring such Cords is the shortest counterexample to an incorrectly narrowed profile.

## Required profile contract

Append a distinct capacity parameter, e.g. CORD_TERMINALS, defaulting to CORDS. The full default uses Cord-indexed stock and preserves arbitrary routed abnormal topology. A compact selection may use the ordinal in ascending active Cord index of `(source Remote, sink Local)`, computed by scanning immutable specs; no RAM map is required.

Before any mutable kernel bind/publication, validate:

* active counts and all existing routing/topology rules;
* either full Cord-indexed stock is selected, or **no active Cord** has AbnormalTerminal track and the compact stock can represent all actual logical ingress Cords;
* mapping is injective and stable for the kernel lifetime, based on actual prepared CordSpecs rather than a build-time estimate or advertised count;
* topology cannot be replaced without repeating this validation. Normal rearm retains immutable specs and therefore keeps the mapping valid.

A simple contract accepts default STOCK=CORDS or compact STOCK=exact ingress count. Allowing larger-than-needed compact stock is also safe if indexing mode is unambiguous. Ensure STOCK=CORDS takes the full mode even if it happens to equal the ingress count. Missing compact slots for a non-None write must be impossible after validation or refuse before mutating flags/readiness. A noneligible Cord's getter returns None; do not alias a fallback slot.

## Every affected path

| Path | Required preservation |
| --- | --- |
| Three constructors: ordinary `new_with_active_counts`, boxed initializer, boxed table writer | Initialize all external terminal stock to None; validate actual completed active topology before publication. Propagate the new parameter through every scheduler impl and relevant size/admission wrapper. |
| `close_remote_input_terminal` | Preserve exact full-value equality on duplicate close; same bytes are idempotent, different bytes refuse. Preserve normal-versus-abnormal replay distinction and all endpoint/Sign checks. |
| `remote_fanout::close_remote_input_fanout_terminal` | Use the same accessor for its preflight duplicate-close checks; keep preflight before mutation and duplicate target detection. |
| `context` | Read terminal only after current closed/abnormal/empty-queue/recovery conditions. Keep full100-byte terminal, FIFO drain ordering and finalizer semantics. |
| `remote_egress_abnormal_terminal` | Default profile must still return exact retained terminal on routed payload siblings after queue drain. Compact no-track topology cannot generate these local-source terminal writes, so None there is proved rather than truncated. |
| `abnormally_terminate_outputs` | Preserve existing global/default behavior and partial-error behavior. A compact topology may return its existing unrouted semantic abnormal error with the full terminal; do not turn it into a storage error. |
| Recovery / terminal phases | `projected_recovery_for_step` compares its separate unresolved-origin terminal against payload storage; it does not write Cord terminal. `context` suppresses input_abnormal for projected recovery as before. Do not conflate this new stock with unresolved-origin or phase storage. |
| Region / DAG / schema/native fast paths | Inspected paths advance through original context/step machinery or inspect queue/closure state; no additional Cord-terminal writer exists. Keep compact profile validation on all preparation entrances, including execution-region configuration. |
| `cancel` | **Do not clear terminal stock merely because queues/values are cleared.** Current cancel retains producer_abnormal and abnormal_terminal while marking producer_closed. Remote egress inspection has no cancelled guard and can still expose a previously retained terminal on the default routed profile. Preserve this exact behavior. |
| Prepared-session `begin_next` | Currently replaces each CordState with EMPTY while preserving sequence. Explicitly clear split terminal stock here too; stale abnormal bytes must not affect replay/equality or later normal invocations. Existing drain/refusal and optional-owned-boundary rules stay intact. |
| Retirement / failure / retry | Retain storage until the original owner retires/drops or the existing explicit reset occurs. No new allocation, refcount manipulation or mutable payload sharing is needed, so this split need not introduce the earlier fanout-transaction hazard. |

## Expected ARM geometry — estimate only

Given the reported current CordState144 bytes on ARM, the remaining fields fit an expected40-byte state after removing Option<CanonicalValue>. CanonicalValue's explicit len+100-byte array has101-byte payload layout; expected Option layout is102 bytes. Rust layout is not a stable ABI guarantee: **measure these exact target types before accepting any number.**

For95 Cords and3 eligible logical ingress Cords, expected backing changes from95*144=13680 to95*40 +3*102=4106 before containing-struct alignment, a nominal9574-byte reduction (likely9568 after8-byte alignment). A31536-byte selected kernel would then be roughly21968 bytes. Treat these as sizing hypotheses until ARM size_of and real ELF/CPU preparation receipts confirm them.

Default full stock may itself save a small amount by avoiding each Cord's padding, but preserving default semantics is the requirement; its size need not match the old binary. Resident static owners, original Root, equipment, allocator rounding and stack are not reduced by assumption. CPU requested peak/retained must be remeasured rather than subtracting this estimate from a different ABI receipt.

The selected generated `workset_0_layout.rs` inspected contains zero occurrences of AbnormalTerminal. The asserted3-ingress count must still be independently derived from actual prepared CordSpecs by the owning Source proof; textual absence alone does not authorize the compact runtime profile.

## Minimum qualification before adoption

1. Differential default-versus-compact traces for no-track topology: normal and100-byte abnormal ingress, queued payload then terminal, equal duplicate close, differing close, malformed endpoint/Cord, and fanout mixed already-closed/open targets.
2. Include local Fore logical ingress with zero remote-lifecycle Sign budget: terminal data must survive without inventing Line authority.
3. Refuse compact stock for any active AbnormalTerminal track and for too many actual ingress Cords, through **every** ordinary/boxed/table-writer preparation path; unchanged default must retain routed local/remote fanout and full terminals.
4. Cancellation followed by default remote egress inspection, ordinary drained rearm followed by new normal/abnormal closes, and recovery/finalization tests must preserve existing results.
5. Capture ARM geometry and exact selected Source preparation costs with unchanged grants. This is a focused6341 reduction, not proof that admission/Start/application or whole target RAM fit is delivered.
