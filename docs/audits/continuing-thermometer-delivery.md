# Continuing Thermometer: delivery and acceptance

This is a delivery plan for the existing environmental instrument, not a new
MCU architecture or memory epic. The four requested outcomes remain unchanged:
independent sampling and timed display; durable same-body history and recovery;
Wi-Fi provisioning and fallback; and restart continuity with browser/Pi joining.

## Current recorder checkpoint, 30 October 2026

The current delivery branch is `integrate/thermometer-dev-sync-20261029`,
containing accepted `dev` through `e049ecc`. Its recording changes preserve
producer-observation time separately from acquisition time, bind the selected
Back's publication observer to the exact committed content, and stage compact
observations through the existing prepared Linux recorder. The supported
`cargo xtask check recording-checkpoint` passes 83 distinct tests and scoped
strict lint at `eb7d6be774`. This includes real file publication/reopening and
separate-process installed-seed recovery, with explicit CPU fixtures for
producer authentication, Host admission, placement and grants. It does not
establish a live selected environmental recorder or acknowledgment eligibility.

A read-only production verification on Forebrain accepted Blue's existing
signed epoch through sequence 654. The continuing collector was still running
in its original process after more than 21 hours. Retained signed evidence and
a current/history projection remain distinct from committing those observations
into the Body's selected recording.

The next integration belongs to the existing joining/recording lane (#6461),
using Root-owned workset renewal. `admit_plot` and complete verified proposals
require the authoritative Lulled Body, current membership and real reservations;
a joined observer binding and operator-selected disk directory cannot substitute
for those facts. Candidate preparation may overlap the old sampler/Mask Play,
but current lifecycle APIs do not authorize adding a Plot while that old
body-wide Play remains current. Extend the existing retirement, checkpoint,
proposal and worker-start transaction with the selected Linux recorder; retain
the same Body and evidence. Do not create a second Body or infer execution
authority from matching historical producer identities.

## Admission preparation and next worker handoff

At `8f87c068f2`, routed admission encoding borrows complete advertisements
rather than cloning their decoded definitions. A sound canonical-byte lower
bound also refuses provably oversized structured payloads before allocating
expanded JSON. Wire fields, compression, decoding and all limits are unchanged.
CPU measurements against retained complete Motherbrain/Threebee advertisements
show 496-byte peak allocations for this refusal path, compared with roughly
1.54 MB for a clone-plus-JSON baseline. These are refusal measurements, not
successful worker admission or physical Root workspace measurements.

Invitation membership completion now accepts a borrowed Host/Boot/generation
scope through the same admission and authoritative lifecycle transaction. The
existing complete-advertisement entrances delegate to that implementation.
Signature transcripts, attempt/replay accounting, archive staging and atomic
publication retain their existing checks. This does not install current offers,
reserve resources, create a Plan/Play or remove the native observer guard.

The next wire integration must be an explicit membership-only document and
receipt, without constructing an empty advertisement for a real worker. Full
initialized equipment must subsequently be resolved losslessly under an
explicit bounded profile before planning. Unknown or mismatched immutable
definitions require complete bounded transfer or refusal. Current Host/Boot,
offer generation, initialized resources and realization provenance remain
actual runtime facts. Membership by itself cannot unblock protected Lines,
worker execution, selected recording or final shared Face delivery.

## Zenoh Line-base evaluation, 30 October 2026

The user has authorized evaluating Zenoh/Zenoh-Pico as a replacement Line base
within this delivery lane. This advances the canon's dormant candidate to an
experiment, not accepted architecture or a delivered adapter. Preserve authored
Plots, Body identity, typed Cords, exact Plans, finite admission and authority.
Do not extend the bespoke transport while its replacement is being evaluated;
capability-definition retention and membership separation remain independently
necessary.

Start with one MCU and one Linux router, pinning upstream source and enabled
features. Prove an actual outbound connection through a named NAT boundary,
router/link interruption and automatic reconnection, continuous local sampling
and display, and exact buffered-history recovery without duplicate commitment
or premature evidence retirement. Measure complete firmware flash, steady and
peak RAM (including stacks and transport buffers), preparation allocation/work,
and Linux router preparation/residency. A host library size or upstream board
demo is not MCU footprint or hardware proof.

Zenoh discovery contributes planning evidence only. Keep Conduit's protected
session, Host/Boot/Body checks, signed provenance, cancellation, stale-generation
refusal and exact Cord framing above the carrier. Zenoh delivery acknowledgments
must never authorize recording retirement; durable commit acknowledgments remain
separate. Use host descriptions for router/transport implementations and finite
resources, with current reachability as runtime evidence.

Removal candidates are the custom rendezvous byte relay, HTTP history polling,
selected-peer lookup and transport reconnect/backoff. `history_relay`'s directional
Cord/session checks and `owner_reconnect`'s job/sequence continuity remain.
Wi-Fi association, DHCP, credential selection, AP fallback and the requested
`.local` provisioning surface also remain; a session library cannot replace them.
Victus owns the pinned Linux router experiment; Forebrain owns MCU adaptation
and footprint/history evidence. Adoption and deletion require these measurements
and the actual interruption/recovery proof, through the existing delivery lane.

The current MCU images use no_std Rust/Embassy with esp-hal or CYW43. Upstream
Zenoh-Pico 1.10.2 has ESP-IDF and Pico SDK/FreeRTOS platforms plus an external
platform interface. Its board examples do not directly integrate this executor,
network stack or admitted allocator. Evaluate a bounded external platform/carrier
seam before replacing working firmware; do not import another OS to satisfy a
transport example.

## Historical source and proof boundaries, 28 October 2026

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

Blue now runs `e3374b324f9f6c365acb754491472df509abe852`. The combined
sampling/Mask startup test passed with unchanged 118,768-byte Root and 73,728-byte
SDK grants and a 36,900-byte static stack. Its signed full network snapshot was
retained by the authorized Linux member; the actual typed acknowledgment produced
a retained Root-signed Pending receipt. During that exchange the MCU reached six
fresh readings and 132 clean display updates. This is raw journal custody, not
installed selected recorder State.

The longer run subsequently reported a display wire NACK after 164 updates; low
clock settling alone is not a proved resolution. The user power-cycled Blue.
Later capture showed 212 clean updates, but its new observation archive had
refused capacity. Signed retirement freed an epoch; that running Play could not
truthfully restart its journal at sequence one. A controlled restart restored
recording through a fresh admitted scope: two committed readings, 50 clean Mask
updates, Troys attached, and the same pinned Body resumed. Automatic in-Boot
capacity recovery is prepared separately, not physically accepted. A later capture
reached 27 committed readings but reported `mask-present-wire-nack` after 66
updates; the short clean run does not establish display durability.

The user confirmed Blue's display VCC is 3V3. The Titan TM1637 v2.4 datasheet
specifically recommends 5 V for blue LEDs and characterizes electrical timing at
4.5–5.5 V. This is a supply-envelope mismatch to investigate, not proof of the
fault's cause. Directly moving VCC to 5 V is not a qualified repair: module
pull-ups and ESP32 signal voltage limits require inspection and suitable level
translation. The datasheet specifies no maximum clock-high/low timeout, so an
interrupt-induced controller timeout remains unproved.

The next combined candidate (`0ac6b9a6c`, ELF SHA-256
`aa3c2636850b83dad716b0d88afefc42587b1000e49c68f4310be369e8e49c1f`)
compiled and packaged within the existing grants and flash partition. It has
not been installed. The private biography-maintenance plan preserves the exact
Clean cursor, requires unchanged observation/network bytes and the original
dense bank on readback, and requires a newly compacted bank before normal Boot.
Its offline preparation has performed no hardware actions.

Cold network recovery is physically proved: all eight 4 KiB journal slots are
blank after recovery, while the entire 655,360-byte observation region and Body
banks were backed up. The actual production payload decoder independently pins
the Root/Body, loads generation 621/schema `root-restart@4`, authenticates the
Clean cursor, and checks its original member-receipt scope and exact epoch
increment. The physical Body banks contain 26,402/26,064 encoded bytes, exceeding
the existing installer's conservative 26,000-byte gate. Do not bypass that gate
or fall back to a pre-cursor firmware image.

The first focused shared-memory extraction, [PR #6536](https://github.com/dancxjo/conduit/pull/6536),
merged into `dev` as `25ad835abf2428fb39d1bdcc2a49d4a4d81400cd` on
28 October. Candidate run `40751736116` passed; exact merged-source Integration
`40757644728` remains in progress at observation. The change reuses exact
immutable common Unit definitions with full decoding fallback and excludes its
static table on AVR. It does not prove AVR firmware admission or complete #6341.
[PR #6537](https://github.com/dancxjo/conduit/pull/6537) separately extracts only
the shared TM1637 STOP/low-settling changes; its physical-fault limitation is
explicit, and its Candidate checks remain in progress.

## Pending-custody preparation checkpoint

The `39f04c6d3` candidate linked with unchanged Root/SDK grants and a
36,900-byte static stack against the existing 36,864-byte floor. Its pointer
lease reduced the CPU pending core from 2,520 to 824 bytes. Physical startup
exposed a separate bank-placement regression: moving the Store backing from
RTC memory into the Root heap left 30,420 bytes free where network preparation
requires 24,576 + 5,848 = 30,424. Admission repeatedly refused before network
construction. Sampling and the Mask nevertheless continued: by 164,934 ms,
the capture contained two observations and 50 Mask consumptions, without a
reported sampler or Mask failure. This candidate is not a passing Wi-Fi build.

The `5a2f633bd` correction keeps the one Store backing in a Boot-initialized
RTC once-cell and transfers its unique reference. Native tests and linking pass;
physical network preparation succeeds and attaches by 29,382 ms. However, its
Mask reports a wire acknowledgment failure by 44,459 ms after eight updates.
That is negative sustained-display evidence, not a passing appliance test. The
capture was stopped early to restore the previously verified firmware. No
network custody acknowledgment or erase was requested on either candidate.

The attended Linux acknowledgment caller is implemented separately. It retains
and reopens the exact signed request before sending, retries those same bytes,
verifies the Root-signed committed receipt, and retains both receipt encodings
with file/directory synchronization. Thirteen custody tests, six join regression
tests, the binary HTTP test and the actual CLI build passed. No physical network
acknowledgment or reclamation has been proved by these checks.

The attended entrance is `cargo xtask prove environment ack-network-snapshot
BINDING CONTINUITY_PRIVATE PEER PRIVATE_STORE OFFER_JSON SNAPSHOT_BIN
PRODUCER_BOOT` (one command). Inputs are private file paths and an explicitly
pinned producer Boot; credentials are not command-line values. The snapshot
must already have been exported and verified. A successful command proves the
retained signed custody receipt, not physical reclamation or installed recorder
State. The bounded retry window spans the live transaction's 900-second deadline;
individual flash-operation and sampling deadlines remain unchanged.


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

## Native Pico membership remains unfinished

Linux observer return and browser WebCrypto return have earlier physical proof,
but neither makes White, Yellow or Green a member of the instrument's Body.
Their current USB/radio firmware has no Body admission receiver. Blue's joining
endpoint currently accepts empty observer advertisements and refuses real
provider/resource advertisements. This is a software integration gap, not merely
a missing Linux Raspberry Pi test.

The smallest native journey starts with Yellow's DHT11/TM1637 profile: authenticate
an invitation for the existing Body, admit its finite real offers, prepare the
same checked sampling/display workset, transfer a bounded assignment, bind its
actual providers and admit both local Plays. Disconnect and power-cycle proof
must preserve Body/part identity and demonstrate local continuation. Existing
Pico native admission proofs, indexed planning, bounded assignment transfer and
hardware owners are reusable pieces; their separate demo runners do not already
prove this journey. BME280 and HT16K33 profiles follow the same contracts.
Observer-only admission would be an intermediate test, not the four-instrument
Body outcome. This remains in the existing #6457/#6461 acceptance lanes.

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
