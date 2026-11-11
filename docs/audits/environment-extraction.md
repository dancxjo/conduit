# Environmental Body extraction inventory

Source audit, 11 November 2026. This is a decomposition and dependency audit,
not hardware, installed Play, recorder, or release acceptance.

The comparison is current `dev` at `5e0d01d0a156cd17002a9714e1a33677547889c8`
and the environmental integration checkpoint at `a28a11877c7bd3424b41de1284e098f990f1d7cc`.
The latter is draft work under #6837; its capabilities and dependencies must not
be attributed to `dev`. SDL extraction is independently owned and is not part
of this change.

## Intended boundary

`bodies/environment` should contain the application's authored workset, Body
construction, application-specific configuration, examples and acceptance
documentation. Its Plots describe sampling, retaining, publishing and presenting
typed observations. Hardware attachments belong in checked Host descriptions;
device protocols, semantic contracts, bounded implementations, storage and
transport custody belong in their reusable Conduit layers.

Rust implementations may remain necessary for physical effects and bounded
execution. Moving application policy into a differently named Rust package is
not Conduitese composition. Conversely, Source that calls a Kind does not prove
that an initialized Back, resource grant, Plan or Play exists.

## Remaining responsibilities

| Checkpoint location | Responsibility and disposition | Existing acceptance owner |
| --- | --- | --- |
| `runtime/src/observation*`, `runtime/build_observation_source.rs` | DHT11 observation contracts, Forms and quantity packing. Keep device decoding with `plots/device-protocols` and the selected device implementation; expose semantic observations through shared contracts. Do not make the product compiler import a Body to learn a device Kind. | #6457; existing device protocol/catalog implementation |
| `runtime/src/publication*`, `canonical_storage.rs` | Bounded retained values, event sequencing and commit staging. Separate generic typed retention from the four environmental outcome channels. Reuse kernel commit/pressure/cancellation; no publication scheduler. | #6457, #6341; shared data/kernel owners |
| `runtime/src/appliance*`, `construction*`, `build*` | Application driver enum, hard-coded Source assembly and preparation sizing. Keep the sampling workset authored; shared Make/lowering consumes explicit checked Source, entries and a selected catalog. Do not promote `compile_default()` and its environmental include list into a general compiler. | #6447, #6457, #6341 |
| `runtime/src/recording*`, `profiles/recording.conduit` | Observation encoding, finite chunks, snapshot references and publication tracking. Keep domain Forms where appropriate; use shared typed record/snapshot/append machinery and exact committed receipts. A signed outbox is not the selected durable recorder. | #6354, #6358, #6359, #6360, #6465 |
| `face`, `runtime/src/retained_publication_face*`, `publication_face_custody*` | Environmental quantities/status and their Face projection versus local lifetime/read custody. Shared domain semantics and shared custody are separate extractions. Preserve provenance and last values independently of freshness. | #6461, #6374, #6409 |
| `history/src/{contribution,graph,browser,native}.rs` | Recorded-observation projection and paired graph realizations. History truth stays upstream; generic series, scales, gaps, units and Looks should be consumed by ordinary Masks. Current `HistoryGraph` hard-codes temperature/humidity and `environment/history@1`; relocating it would not produce a general chart. | #6461, #6465; shared Face/Mask owners |
| `http/src/{lib,mask,show_mailbox}.rs` | Global environmental publication, bespoke HTTP representation and finite Show custody. The mailbox's lifetime/withdrawal mechanism is reusable; the HTTP Mask's field selection and units are domain presentation. Do not turn this proof's static publication owner into a production Body service. | #6461, #6374, #6409 |
| `runtime/src/member_receipts*`, `host/src/member_binding.rs` | Signature/identity verification and securely reopening pinned evidence. Locate active membership protocol/custody in Body/Host machinery. First check against the newer installed-member path; retire obsolete duplicate evidence protocols rather than making both permanent APIs. | #6340, #6409, #6461 |
| `make/src/display_output.rs` | Checked TM1637 output composition and planned authority. Device protocol Source and selected output Back belong in shared protocol/Make layers; the application's choice to use that display remains authored. | #6378, #6457 |

The checkpoint contains roughly 11,454 source lines in `runtime`, 1,686 in
`history`, 1,155 in `face`, 848 in `http`, 529 in `make`, and 502 in `host`,
against 102 lines in seven profile `.conduit` files. These are inventory counts,
not an instruction to move every Rust line into shared production code. Tests,
proof adapters and old fixtures need their own disposition.

## Actual dependency problems

At the draft checkpoint, `products/conduit/src/plot_source/device_catalog.rs`
imports observation, recording and publication contracts from
`conduit-environment-runtime`. MCU source preparation and execution also depend
on that package. The production compiler's device catalog dependency does not
exist in the inspected `dev` source. It is a delivery boundary to repair before
promoting that draft, not a defect to attribute to released software.

The reusable environmental Face package exists on both sources. This slice
moves it to `semantics/environment-face` as a normal workspace package and
updates its real consumers, build tracking and focused proof entrances. The
existing API and implementation are retained without a forwarding crate at the
old path. The integration branch's extra borrowed-preparation methods must be
preserved when this extraction is adopted there.

This first move does **not** generalize the fixed `environment` subject, add
pressure sensing, make Face contribution an admitted Plot, or turn two graph
renderers into one shared Mask path. Those remain explicit follow-up work;
existing contribution and preparation tests cannot prove those outcomes.

A subsequent slice extracts the owned canonical selector from
`runtime::PreparedCaptureSelector` into the shared semantic catalog as
`PreparedCanonicalSelector`. Its preparation and execution algorithms are
unchanged: preparation owns the exact selector, validation and bounded output;
selection distinguishes a matched value, an explicit drop, a refused unmatched
variant and malformed input. Environmental kernel composition, display planning
proofs and the Pico proof now import the shared helper directly. There is no
forwarding export from the Body runtime. A musical control-event test exercises
the helper without the environmental runtime, including allocation-free
selection, rejection and output pressure.

The integration branch additionally supports linked/static selector storage.
That optimization is not present on the inspected `dev`; it must be preserved
and reconciled into the shared helper when adopting this extraction. Do not
replace its static preparation with a decoded Type tree or infer that the new
owned-helper tests qualify that separate path. The std selector realization's
placement/authority checks and its different prepared-storage policy are outside
this slice.

TM1637 output planning is another distinct shared responsibility. It moves from
the environmental Make library into
`mechanisms/implementations/tm1637-output-make`; its authored Plot is now
`plots/device-protocols/tm1637-output.conduit`. Its input remains a typed frame,
not a temperature or a page-selection instruction. The same ordinary planner
selects actual Host offers, exact local Fore and independently supplied write
authority. Its tests retain shared expression/selector implementations directly,
so neither production nor test dependencies require the environmental runtime.
The environmental Make package retains only its application specification proof.

The Plot's name changes from `environment-display-output` to `tm1637-output`;
newly prepared Source/Plan identities change accordingly. No alias or identity
compatibility claim is introduced, and installed firmware is not rewritten by
this source extraction. The existing environmental display-output proof entrance
now includes the shared package alongside application frame/packet checks.

The DHT11 observation realization moves to `mechanisms/devices/dht11` as a
`no_std` shared device package. The protocol and quantity Forms remain authored
in the existing device `.conduit` files. Optional kernel-step and source-capture
features retain the original conversion Back and checked request geometry;
neither installs a physical provider. Application publication borrows the one
shared immutable result schema. Device-only Forms/frame tests move with the
implementation while the Face/Mask integration remains with the application.
Direct consumers replace the Body-local observation module; no forwarding
module hides an unconverted caller.

The inspected integration draft has additional packing/static-storage APIs not
present on `dev`. Preserve those at adoption, just as for the Face and selector
extractions. This does not generalize the DHT11 contract into a promise that
every environmental sensor has the same fields or encoding; other devices
provide their own truthful observation realization.

## Authored construction gap

The inspected `make/body/src/construction_source.rs` accepts Body identity and
Host declarations (plus the separately handled Mask policy); its checked example
names one MCU and an optional browser. It does not implement an arbitrary central
Host-and-Line topology/workset language. Do not publish invented `line = ...`
syntax as runnable Conduitese. Extend the existing construction/check/Make
entrance with its owning contracts when that capability is implemented.

Desired Host descriptions name physical attachment, initialized Base choices,
finite resources and fabrication choices. Observations carry semantic quantities;
observe/sample/retain/present are operations. A temperature need not become a
hardware-specific `environment/current-temperature` Kind. Discovery and runtime
counterevidence inform planning; neither becomes execution authority.

## Delivery order and completion evidence

1. Separate shared domain contracts/projections from the Body and reconnect all
   consumers through the supported proof entrances. Prove hosted behavior and
   relevant `no_std` compilation without increasing recurring allocation.
2. Replace the draft compiler/MCU dependency on Body-local runtime code with
   shared explicit Source/catalog/implementation preparation. Prove another
   workset can use it without environmental defaults.
3. Reconcile typed retention/recording with the existing recorder owners. Prove
   actual committed identity, restart and acknowledgment recovery, not just codec
   equivalence or a signed transport batch.
4. Compose per-producer observation/history Faces and shared graph Looks through
   ordinary Mask routes. Prove multiple Hosts, gaps/staleness and units on both
   browser and native output with the same semantic input.
5. Retire superseded proof-only services and protocols after their replacement
   consumers are verified. Keep the application workset and Host construction in
   Conduitese, with truthful capability/refusal and physical acceptance evidence.

The extraction is complete only when shared production layers no longer import
the environmental application, its authored files determine application
composition, and the resulting installed Plots/Plans/Plays and shared Masks are
verified. A directory relocation or an isolated green test is not that gate.
