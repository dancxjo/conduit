# Continuing Thermometer: delivery and acceptance

This is a delivery plan for the existing environmental instrument, not a new
MCU architecture or memory epic. The four requested outcomes remain unchanged:
independent sampling and timed display; durable same-body history and recovery;
Wi-Fi provisioning and fallback; and restart continuity with browser/Pi joining.

## Source and proof boundaries, 28 October 2026

[PR #6457](https://github.com/dancxjo/conduit/pull/6457) is still a draft at
`04f6f3d46efcfd49f7fbc0b9486c4f1d96cd05fb` on
`work/mcu-appliance-birth-20261024`. The reviewed GitHub observation found 449
changed files, 53,154 additions, conflicts with `dev`, and no status-check rollup.
The new instrument work is on `work/esp32-metadata-20261026`; its latest pushed
checkpoint when this inventory was written is
`c65f51bdee2f9f604b3a778189f0f16c7b99d372`. These are different review surfaces.
Do not attribute new physical results or current-source validation to the old PR.

The fetched `dev` source was `9e0f3dd6876031df84e872aa29a6bc965a1b984a`, with a
successful Integration run `40703513797`. The instrument branch had 392 commits
absent from that `dev`, while `dev` had 256 commits absent from the instrument
branch. Refresh useful source onto current `dev` in small changes; do not merge
this accumulated ancestry or restore historical captures.

Blue's installed source is `c59d143463d4ce78f01f57826629d0a8637056ff`, not the
latest branch head. Its physical install resumed the pinned Body on a new Boot,
ran both local workers, completed two sensor observations and 50 Mask updates,
and attached to Troys. The observed Root grant remained 118,768 bytes; steady
free space was 23,424 bytes. The user separately confirmed clean real readings
and display transitions on the preceding display fix. Subsequent read-only
queries show that sampling continues. This establishes useful physical progress,
not an accepted release or every interruption/restart scenario.

The same instrument's complete 32 KiB network journal was exported in 64 signed,
pinned chunks, retained with fsync/reopen, and compared byte-for-byte with the
fresh pre-install backup. All eight pages matched; no acknowledgment or erase
was requested. Snapshot SHA-256:
`6a784123662275da01bd75c7dd53f21a4c525764c9c09f92c0e4f7d0b1a390f2`.
This is raw evidence custody, not selected recorder State or journal reclamation.

## Reconcile the ten observations

| Concern | Current assessment | Remaining acceptance |
|---|---|---|
| Real measurement | Blue acquires typed environmental observations physically. | Keep exact Source, Plan, Boot and sample-time provenance through recording; no bench substitution. |
| Physical display | Combined local sampling and timed Mask workers now execute on Blue; the pre-planning RAM failure is no longer the current blocker. | Reconcile the local indexed/native route with #6374/#6378, prove complete admission refusal, and rerun clean display progress during current-source outages. |
| Several hosts, one Body | Blue and the authorized Linux collector have demonstrated shared-body custody. | Do not call Pico radio relays body members. Prove independently authorized joining on browser and Pi hosts. |
| Disconnected continuation | Earlier physical owner/carrier experiments remain useful with their original source/Boot scope. | A newer recorder-outage capture explicitly had an electrically refused Mask. It proves measurement/history progress, not successful display progress. Repeat the full journey after the display fix. |
| Durable journals | Local observations, signed exports and finite journal ownership exist. The network journal is currently full. | Commit custody, reclaim safely, exceed eight network retirements, recover after cuts, and keep local measurement running throughout. |
| Authority and provenance | Canonical membership verification, exact scopes and signed custody have deterministic checks. | Qualify actual cancellation, lost acknowledgments, stale member/Boot generations and retirement on the complete live route. |
| Compiler/runtime representation | Shared borrowed facts, compact plans, immutable definitions and streamed identities have implementation and focused tests. | Extract independently reviewable changes onto current dev; preserve ordinary semantics and unsupported-path fallback. |
| Resource budgeting | Both workers now fit the admitted ESP32 Root. The pending transaction uses bounded backing. | Measure overlap on the actual linked target; refusal must precede effects. Do not treat CPU size or a native build as a physical workspace proof. |
| Pico target | White, Yellow and Green have useful physical sensing/display implementations with distinct hardware profiles. | They are not yet equivalent autonomous Conduit hosts. Prove the same checked workset, persistent Wi-Fi recovery and membership separately for each supported profile. |
| Browser/recording | AP form submission, signed collection and some returning-browser paths have separate evidence. | Finish independent invitations, Pi joining, installed recording and fresh-process recovery through their existing owners. |

## Existing owners and delivery slices

Use at most two overlapping implementation PRs, with at most one merge-ready
shared-core change. Keep the runnable instrument as the integration anchor.
No new umbrella issue is needed.

1. **Land a small shared-memory correction first.** Extract `f3ffa3e4e` plus
   `ebdc08aac`: reuse exact immutable common Unit definitions, preserve full
   fallback for any differing byte, and exclude the static fast-path table from
   AVR SRAM. Current-dev files matched the extraction base exactly. This is
   supporting #6457 work, not completion of #6341 or compact Quantity-region
   specialization #6016. Include the coherent codec extraction required by the
   current file-size policy, and validate current-dev Core semantics.
2. **Extract borrowed offer/admission preparation next.** Audit the dependency
   chain before opening another shared-core PR. #6341 owns the actual setup
   workspace-custody contract; retained offer schemas and overlapping preparation
   must be accounted for under that contract. Representation savings alone do
   not discharge its refusal, retention or retirement criteria.
3. **Deliver the autonomous two-Plot instrument.** #6457 with #6374/#6378 owns
   installed local execution and physical renderer custody. Gate delivery on
   power-on admission of the complete sampling/display workset, current-Body
   identity, real measurements and timed display without another host, exact
   shortage refusal, and clean disconnected progress on the same candidate.
4. **Finish finite network-custody recovery.** #6340 and #6409 remain concurrent
   lifetime/authority gates. The acknowledged snapshot must be durably retained
   by an admitted member before Pending custody commits. Root and the sampler
   share one reviewed flash arbitration domain. Hold the actual frozen snapshot
   and exclusive Store through every step; release only after completion or
   explicit failure fencing. Prove lost-ACK replay, interrupted checkpoint and
   erase, Clean readback, renewed capacity beyond eight retirements, and recovery
   without discarding new evidence. Current CPU/native preparation is not a
   physical success claim.
5. **Finish Wi-Fi product behavior.** Preserve the authorized private credentials;
   demonstrate AP form -> selected station -> `.local`, saved-network selection,
   automatic reattachment, and AP fallback on unsupported/unavailable networks.
   Treat host NIC connectivity, Pico radio transport and MCU persistence as
   separate facts. Verify greenhouse RF coverage rather than infer it from an
   indoor Troys connection.
6. **Complete recording and joining through existing lanes.** #6461 owns the
   authorized browser/native journey and recorder integration. #6465 owns exact
   commitment by the selected recording Back. Keep #6354 typed file-effect
   correlation, #6358 installed durable State/restart, #6359 authored catalog
   publication, and #6360 admitted Linux checkpoint execution distinct. Do not
   absorb those implementations into this firmware branch. #6295 remains the
   environmental-product acceptance owner.

## Current recovery work is prepared, not delivered

The shared checkpoint writer advances one bounded erase, program or readback
operation per admitted step. It authenticates the full old source and complete
candidate before effects, checks complete emission replays, verifies physical
readback, and commits last. Its production bridge owns the actual frozen
snapshot and Store lease. Cancellation after a possibly effective physical call
fences the Store until restart.

Focused evidence: eight low-level checkpoint tests including 57 media cuts;
maximum 32,608-byte output with transaction backing below 1 KiB; 52 ordinary
restart tests plus an isolated Boot-global transaction test; five Linux
retention tests including byte/transcript agreement with the actual MCU ACK
codec; and native ESP32 compilation. The isolated test is intentionally separate
because another fixture owns the same global Store. CPU memory figures are not
native workspace qualification.

The Linux retention module currently has no posting caller. The HTTP framing
route has no live Root scheduling on Blue. The newly drafted shared-domain
extension and ACK queue must be validated before integration. No result here
claims an installed recorder, live acknowledgment, physical Pending/Clean
reclamation, or a completed all-host Body.

## Review and merge gates

For each extraction: pin current dev and the exact useful delta, check ownership
and open PR overlap, run focused behavior/refusal tests and owning-package
checks, inspect the diff for generated evidence, and publish a small PR to dev.
Use the exact required Candidate checks before merge; keep merged-source
Integration and publication separate. Do not manually push to dev/main or close
parent issues from branch-local evidence.

The full goal remains active. The immediate chunk ends with reviewable shared
memory work landed through the normal dev pipeline and a source-bound acceptance
inventory for the autonomous instrument. That checkpoint does not replace any
of the four original product outcomes.

## Generality and program evolution

Universal semantics do not require every host to retain the compiler, planner,
all drivers or all persistence services. A proposed compact/tiny profile can
accept a finite, checked target artifact, verify its declared envelope and bind
actual hardware locally. It must still uphold identity, types, ownership,
lifetime, scheduling and refusal for admitted operations. This describes a
required boundary, not an implemented universal tiny-host loader.

Some current changes are broadly reusable: borrowed immutable offer facts,
streamed canonical identities, and separating preparation from bounded kernel
execution. Other changes need a stronger generality test. The five built-in Unit
lookup is exact and retains general fallback, but that alone does not prove the
fallback's execution stack fits every target. An artifact-specific image recipe
is legitimate construction specialization only while the kernel and verifier
remain independent of the environmental graph. The same caution applies to the
local indexed Mask preparation route.

After the present delivery slice, use an unlike workload, such as a finite
button counter or pulse recorder, to test the same checked lowering, admission,
kernel and recovery without new application-specific execution cases. Record
base code/RAM, per-Back costs, state and buffers, immutable shared data, and
phase-specific peak workspace. Adding pressure sensing should have an explained
marginal cost; it is not enough to observe that the total still fits once.

Keep three operations distinct: replace a Plan using already installed Backs;
install an additional bounded executable artifact where a host offers a loader;
and replace trusted firmware, normally introducing a new Boot. A remote compiler
can prepare the latter two without a resident MCU compiler. Activation still
needs authorized scope, actual quiescence and safe output state, preserved Body
and application-state custody, finite transfer/staging, artifact compatibility,
boot validation, and a truthful recovery mechanism. No arbitrary native-code
sandbox is implied by a signature or a checked deployment Plan.

The current roughly 3.3 MB ESP32 image cannot have two complete application
copies in 4 MB flash. Standard ESP32 OTA requires two OTA app slots and metadata;
shrinking the image, more flash, external staging or an independently reachable
programming host have different recovery guarantees. Do not label a single-slot
remote rewrite as rollback-capable OTA. RP2040 BOOTSEL/UF2 provides a useful ROM
recovery/programming mechanism, not by itself an admitted Conduit update protocol.
See the official [ESP32 OTA contract](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/ota.html)
and [RP2040 boot ROM documentation](https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf).

Very small targets need an explicitly smaller finite profile; they cannot admit
an arbitrary-sized proposal or all services at once. An AVR SRAM guard preserves
one existing path, not a meaningful AVR product proof. Unsupported execution,
update and durability must remain explicit refusals. A machine that lacks the
minimum membership/authority mechanism may instead be a peripheral of a host;
calling it a first-class member cannot manufacture that mechanism.

These criteria guide the existing work; they do not introduce a new OS or loader
implementation lane before the continuing instrument is delivered.

## Proposed profile vocabulary

“Tiny” can describe an implementation/resource profile; “moon” can describe a
relationship in which a host uses planning or construction offered elsewhere.
These are independent axes: a large host can delegate planning, and a small
host can possess a bounded local planner. Neither name establishes a new Host
species, authority class, kernel or `is_moon` execution branch.

Keep offered operations, finite admission bounds, update support, retained-state
promises and disconnected behavior explicit. Local Admit/Play/Keep/Lull/Wake
responsibilities remain ordinary Host obligations for whatever is offered.
Using a remote planning service expands what the Body can do; it does not
transfer that service's implementation or authority into the requesting host.
Local acquisition without reflashing requires an already-supported loader and
sufficient admitted resources. This vocabulary is a design aid, not a delivered
conformance profile or a new prerequisite for the thermometer.
