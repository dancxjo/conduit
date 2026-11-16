# Finite MCU history recovery — bounded audit, 2026-11-15

**The existing typed read service can recover committed records without erasing anything. There is no qualified existing selected-recorder-commit → MCU NOR reclamation path in the inspected implementation.** Reading is the next reusable increment; resumed append capacity requires a separately owned generation/erase-block lifecycle, not another acknowledgment boolean.

Read-only source audit: no Cargo, production changes, Root IPC, device I/O, GitHub operations, journal creation or erase. Primary evidence `outputs/blue-root-continuity-20261115.json` shows admitted execution progress over60 seconds. Its `append.recorded=0, capacity=4` means **four Capacity outcomes**, not a four-slot residence: `targets/mcu/host-runtime/src/execution_append_status.rs` counts result variants. The checked Blue Host currently declares1264 slots and8192 prefix bytes. This audit does not independently prove its live media occupancy or recover an actual record.

## Existing reusable read path

`mechanisms/io/resource-snapshot/src/service_contract.rs` defines one `resource/record-service` with independently typed append and read operations over **one affine NOR residence**, not two overlapping flash owners. `targets/mcu/host-runtime/src/recording_service_equipment.rs` recovers the original owner, publishes its actual facts and installs independently original append/read grants. Installation rejects wrong identity/generation, unrecovered residence, zero operation budgets and mismatched operation bounds. Keep this original owner and the existing native flash activity window, which excludes concurrent timing-sensitive GPIO/display operations.

Official read semantics:

* Kind `resource/read-record`; native call `conduit.host/resource-read-record@1`; authority `authority/resource-read-record@1`.
* Service Fore `read-index: Count` (8-byte canonical Count, bounded to u32); `read-result: RecordReadResult` with canonical maximum837 bytes, carrying at most200 record bytes.
* Results `Recorded {index,captured_end,record}`, `Gap {index,captured_end,reason}`, `End(captured_end)`, `Cancelled`, `Busy`, `RecoveryRequired`, `Unavailable`. Gap reasons distinguish vacant, unpublished-or-damaged and corrupt. Read may also refuse output pressure, invalid binding/input/request/capability or terminal state through the service's existing refusal path.
* Each native read captures the current recovered physical tail independently. The client must retain the first `captured_end` as its own finite scan ceiling; later appends may increase subsequent reported tails. `captured_end` is an exclusive **physical index**, neither observation sequence nor time. Do not wait for a moving End to finish a bounded scan.
* `append/read_window.rs::RecordReadWindow` already supplies a fixed-prefix, same-owner bounded scan without batch copies for local callers; it is not serializable, a remote cursor, a read grant or an ACK.

The authored Source fixture at `targets/mcu/source-build/src/software/recording_tests.rs::record_service_retains_its_native_equipment_selection_pin` demonstrates the ordinary wiring: request→store.read-index, store.read-result→typed output, record→store.record and append outcome→status. It proves the Source seam exists; it is not evidence that the live Blue Plan has admitted/exposed those particular Fore ports or a protected Line.

**Exact next path:** prepare/admit an authored finite read Plot using the same selected record service, bind its real typed ingress/egress through the existing admitted Fore/Line machinery, scan original physical indices through the pinned ceiling, and preserve every Gap/error as evidence. Return bytes to the existing selected recorder under their exact provenance. Do not add a USB-only raw-flash/read command or a second storage owner.

## Three distinct commitment boundaries

1. **Media publication:** `append.rs` writes/verifies immutable512-byte slots with256-byte units, hash and final commit marker. Recovered committed bytes remain readable across Boot; damage remains visible. Recovery advances beyond occupied/torn slots rather than overwriting them.
2. **Kernel completion:** `native_service/execution.rs::acknowledge_kernel_completion` checks the exact node/call/request/encoded outcome and advances service request custody; append custody records `kernel_committed`. It does not mean Linux recording, catalog/State commitment or delivery.
3. **Selected durable recorder commitment:** requires exact original record correspondence, actual selected publication/readback/catalog+Body State commit and fresh-process recovery. Only that separate owner can issue producer-retirement evidence for its exact committed identities. Transport receipt, successful read, Face rendering and kernel completion supply none of it.

## Provenance available — and missing handoff evidence

For the current environmental **log format**, `bodies/environment/runtime/src/recording/log_encoder.rs` emits context fragments before samples. Context covers Body, producer Host/Boot/Plan/Play, stream identity, exact observation type digest, first sequence, temperature/humidity/recording-disposition Form identities and clock basis. Samples carry original stream sequence, local acceptance milliseconds, disposition and packed quantity values. The clock identifier is **`producer-acceptance-boot-ms@1`**: it explicitly does not claim physical sensor-acquisition time or wall time.

`recording/log_reader.rs` verifies context framing/digest/expected observation representation, continuity, sequences and nonreversed clock, exposes missing-context/discontinuity and returns original producer scope. Decoded context is descriptive; Root must authenticate the physical residence/producer/history independently. A sample referencing a missing/damaged context cannot be silently assigned the current Boot or current Source.

This preserves meaningful original acceptance time, but does not manufacture physical acquisition time, a wall-clock synchronization relation, raw capture evidence, recorder commitment or authenticated network delivery. Do not retimestamp replay with reception time. Other record formats must be identified from the actual retained bytes rather than assumed to be this encoder's format.

No actual example record or current complete read-route receipt was retrieved in this task. The Face owner's requested single handoff unit still needs:

* fresh lawful Hamm Body/Host/Boot/configuration and exact Source/Plan/Play receipt;
* actual selected typed read Fore and owned Line/session identities, finite request/reply/item/work/deadline limits and exact837-byte maximum;
* pinned physical residence/generation and first captured_end;
* exact authored observation schema and Form identities plus an actual decoded example with original stream sequence, original Boot/Plan/Play and acceptance-clock ticks;
* explicit Gap/pressure/expiry behavior, and a separate wall-time relation if any is independently known.

Initial integration must be **read-only, with no retirement ACK**. A synthetic example or newest live reading is not a recovered record.

## Why capacity cannot currently be reclaimed automatically

`PreparedNorRecordResidence` explicitly owns no generation selection, authority, erase, reclamation, recorder semantics or durable delivery cursor. Its full-residence Capacity refusal is deliberate. `NorStorage` has no erase method because adjacent logical record slots can share physical erase blocks; ESP32 `snapshot-nor` likewise exposes program/read only. `into_storage` transfers original affine custody; it does not make programmed bits erased or grant block ownership.

Existing generalized concepts remain useful but are not a NOR reclaim implementation:

* `architecture/core/src/resource_collection.rs` retains/selects immutable resource-generation identities within finite bounds; identity publication is not physical reuse or erase permission.
* `targets/std/src/hosted_resource.rs` owns one admitted generation and refuses retirement while readers retain leases. A new generation requires its own sealed provider. This establishes custody/lifetime discipline, not permission to erase shared MCU blocks.
* Existing recorder snapshot/Text-generation adapters and `RecordingCommitObserver` preserve exact selected references and committed sequence ranges after actual kernel acceptance. They do not choose replacement generations, publish catalogs, or own MCU erase blocks.

Consequently a durable recorder receipt alone cannot resume a physically full append residence. The missing connection is an authenticated exact-record retirement/cursor policy **plus** an independently admitted generation transition that owns whole erase blocks, proves all live neighboring records/continuation bytes/readers are preserved or retired, survives interruption/reopen, and obtains safe flash execution windows. A new already-erased independently selected residence could be prepared only through that lawful resource-generation owner; resetting next_index, reopening the same bytes, clearing counters, wrapping indices or erasing an arbitrary slot is invalid.

## Existing ownership and next actions

Local references reviewed before considering issue work: `outputs/installed-source-recorder-first-chunk-20261101.md` (selected publication/readback/catalog/Body State/reopen and no producer ACK without correspondence), `bodies/environment/docs/recording-publication.md` (kernel-commit observer limits), and the prior continuing-Body handoffs. These already separate recording and custody obligations under #6295/#6461/#6465, with recorder installation/publication/checkpoint work tracked by #6354/#6358/#6359/#6360 and lifetime/resource constraints under #6409/#6341. No new issue is proposed or created here.

1. Hand the Face owner one actual admitted read-route/provenance unit above and recover a bounded immutable prefix, preserving gaps; do not claim reclamation.
2. Feed those exact records into the ordinary selected durable recorder and qualify readback, catalog/State and fresh-process recovery with lost replies/idempotence. Keep original sequence/time.
3. Reconcile the missing MCU generation/erase-block custody with the existing storage/Root owners before implementing retirement. Until qualified, report append Capacity honestly while local sampling/display continue where their admitted resource budgets permit.
