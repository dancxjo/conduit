# Shared USB protocol plots

Development groundwork for [#4831](https://github.com/dancxjo/conduit/issues/4831).
This directory is below portable application meaning. Its inputs describe USB
wire facts; its calculations neither discover devices nor grant authority.
The hardware boundary follows
[#4833](https://github.com/dancxjo/conduit/issues/4833): Rust owns physics-facing
primitives and invariants; plots own protocols. Controller setup/teardown,
bounded endpoint/control transfers, DMA lifetime, interrupt acknowledgement,
resource enforcement and mandatory local safety may remain below plots.
Descriptor interpretation, class recognition, report decoding, control request
construction, PCM negotiation and framing, Ethernet framing, and protocol retry
policy belong in reviewed plots. An endpoint provider must not conceal a class
driver behind a generic-looking gear.

This USB slice does not absorb #4833's I2C/BME280 acceptance. Portable binary
composition is shared with that completed code slice: checked integer
widening preserves the entire source domain, and packed-word helpers are bounded
groundwork rather than a substitute for reusable bounded byte/frame contracts.
Prepared pure expressions preserve complete structured values and select bounded
byte-sequence fields through the generic exact-type selector. Their finite
output and selection buffers are allocated before Play. Source checking and
prepared execution agree for empty through maximum-length control payloads;
malformed envelopes and substituted Types remain refusals. This is payload
composition groundwork, not native transfer or class-device execution proof.

`hid-reports.conduit` interprets already-received boot keyboard and mouse frames
according to [HID 1.11, Appendices B/C](https://www.usb.org/sites/default/files/hid1_11.pdf).
Its eight-octet profile checks actual count before indexed reads, preserves exact
keyboard error usages, rejects duplicate nonzero keys, and sorts six key slots
with six fixed passes. Mouse interpretation preserves signed motion and the
original wire bytes, including uninterpreted extensions. The standard profile
ignores OEM/constant fields; the separately named zero-reserved profile enforces
the existing stricter keyboard and button policy. The boot motion profile
refuses the out-of-domain `0x80` displacement rather than clamping it.

`cargo xtask make conduitos usb-plots-check` includes these Source contracts.
`hid-mouse-order.conduit` retains two compact observations ahead of pointer
history. The existing decoded wire result and 2,048-byte endpoint payload keep
their own bounds. Packaged preparation issues no authority. Expression proof
preserves signed motion and buttons, distinguishes invalid observations, refuses
duplicate/stale/distant ordinals, and reuses two slots across 128 observations.

`hid-mouse-order-lifecycle.conduit` drains that state through the existing kernel,
seeded state, feedback Zip and finite Merge. Kernel conformance covers held
output pressure, 64 reuse cycles, normal drain, missing ordinals, each physical
failure tag and cancellation. Prepared execution allocates nothing; cancellation
revokes a held observation without fabricating normal completion.

`hid-mouse-pointer.conduit` advances normalized position and event sequence only
for valid ordered motion. It preserves the existing motion scale, coordinate
clamp and primary button mapping, retains history for invalid observations, and
refuses invalid state or sequence exhaustion. The mechanical
`SourcePointerSampleDecoder` converts its checked sample to the existing portable
pointer seam without interpreting reports. Expression and conversion allocation
checks pass. Ordinary two-capture device installation remains pending; these
fixtures establish Source/kernel behavior, not native endpoint execution.

Existing report conformance covers every key-slot permutation and motion octet.
Both class topologies also execute 64 frames through the production kernel with
normal closure and zero allocations during Play. Held output acknowledgements
retain the earlier report under pressure; malformed input emits its exact
observation without a decoded keyboard report. This establishes checked report
interpretation, not press/release state, installed class offers, endpoint/control
lifecycle, device execution, or physical compatibility. Existing native Rust HID
behavior remains migration debt until the complete plotted path earns its proof.

Probe plots consume an already-admitted attachment; recognizing a class never
mints authority or selects a host resource.

`protocol.conduit` currently owns endpoint address arithmetic, interface-class
recognition, boot keyboard/mouse octet extraction, control setup construction,
reusable ring positions/cycles, PCM packet cadence and completion correlation.
Packed words contain little-endian wire octets. The U64 representation is a
finite arithmetic representation, not a physical pointer or capability.
Signed mouse motion is interpreted separately from octet extraction.

`descriptors.conduit` owns bounded descriptor cursor advancement and device
prefix decoding. Cursor results distinguish the end of received data, a short
header/body, malformed geometry, and a transfer exceeding the 256-octet storage
profile. A next offset is emitted only after checking that the complete record
fits the actual transfer. Device decoding uses eighteen reusable storage octets
and an independent actual count; padded storage cannot turn a short transfer
into a complete descriptor. The endpoint-zero packet field remains exact wire
truth, including a SuperSpeed exponent, pending speed-specific interpretation.
These plots check and execute in prepared expression fixtures. The
`device-probe.conduit` topology also runs the eighteen-octet device decode
through a selected native control Back in the x86_64 proof appliance. Full
native configuration enumeration and class-device execution remain unimplemented.

`configuration-descriptors.conduit` decodes configuration, interface and
endpoint prefixes from fixed storage plus an independent received count. It
preserves alternate settings, zero-endpoint interfaces, raw power/interval
fields and all packet-size bits for later speed/class policy. An extended
record must be completely received before its prefix is interpreted. These
prepared-expression fixtures cover short, malformed and oversized data and
repeated buffer reuse. They do not yet walk a full configuration or authorize
an endpoint. Wire layout follows USB 2.0 section 9.6, tables 9-10, 9-12 and
9-13 ([specification](https://www.seriesten.org/docs/protocols/USB_2.0_Specification.pdf)).

`configuration-walk.conduit` checks the entire received configuration and
publishes descriptive interface/endpoint records. Its profile admits 256 bytes,
four interface/alternate records, eight endpoints and sixteen subordinate
descriptors. Alternate settings share a distinct interface count; each interface
needs a default setting and its declared endpoints. Duplicate interface/alternate
pairs, orphan or duplicate endpoints, invalid addresses, truncated records and
inconsistent totals remain refusals. Unknown class records consume the same
finite descriptor budget. Raw packet, power and attribute fields remain available
for later speed/class policy; this parser does not authorize endpoint use.

The walk composes 56 ordinary checked gears inside the existing kernel profile.
Compact intermediate words retain received descriptor offsets, and final U8
fields are selected from actual wire octets without narrowing casts. Every
prepared result fits the existing 4096-byte Fore budget. Deterministic conformance
compares ordinary and prepared evaluation and proves allocation-free reuse of
complete valid and malformed walks. This is complete configuration parser proof,
not native configuration exchange or class-device execution proof.

`configuration-probe.conduit` constructs one configuration-zero request for
256 bytes and preserves each class-neutral transfer disposition. Its completion
framing rejects contradictory counts and short flags before the complete walker
sees a frame. The checked topology has 60 gears and one control call; preparation
publishes no discovered transfer authority. Deterministic production-kernel
execution covers complete, short and stalled exchanges, 64 repeated calls, and
normal closure with reusable value slots. Allocation counting covers the entire
Play, including admission, pure dispatch and output acknowledgement, with zero
allocations across 64 exchanges. The pure-expression contract admits sixteen
repeated instances; planning refuses a seventeenth. Every instance retains its
own finite storage admission.

`cargo xtask make conduitos usb-configuration-proof` executes the configuration
Source over the actual selected x86_64 control machinery. The retained emulator
receipt requires 64 observed and decoded exchanges, exact Source/Plan/Play and
attachment identities, multiple complete ring-cycle transitions, and normal
closure. Its separate appliance admits a 256 MiB preparation arena, 512 MiB
emulator memory and a two-minute preparation allowance. The configuration receipt
has its own schema and file; the ordinary prepared USB, keyboard and rescue
image keeps its earlier profile. Preparation and Play storage remain distinct,
finite admissions. The fixture retains legacy attachment setup and a cooperative
proof grant. It establishes configuration exchange, with no class-device or
physical compatibility claim.

The register-leaf groundwork in `src/machine_membrane` validates existing opaque
Base possession before an aligned, bounded 32-bit access. Native composition
must bind the exact already-admitted device-memory mapping and revoke possession
before unmapping. Its cooperative fixture is not hostile-code containment. CPU
ordering is distinct from mapping attributes and DMA cache maintenance.

`plots/machine/register32.conduit` builds the primitive requests and calls the
register Kind through ordinary checked topology. Revision 2 of the register Host
Call takes exactly 16 canonical U128 bytes: offset, value, operation and reserved
bits; its result is exactly four U32 bytes. The provider rejects malformed words
and wrong node/call bindings before access. Production preparation binds a sealed
Plan fragment, canonical active Play, selected Base/provider, authority/resource
and the complete ordinary numeric lowering, including its call, routing, storage
and terminal tables. The kernel Host Call back is shared with pure expressions;
there is no separate machine scheduler. Fixtures prove repeated calls, output
pressure, typed denial, revocation and rejection of late cancelled completions.

A register window alone does not confine DMA or enforce mandatory device safety.
The trusted native mapping owner must enforce those invariants independently.
USB class plots consume class-neutral bounded transfers; they must not be given
arbitrary controller-register programming authority.

`src/usb_base/control_request` validates an immutable eight-octet setup word
and a borrowed bounded OUT payload on every architecture. It preserves class
opcodes and request parameters without probe policy. Direction/length checks
reject extra IN payload and mismatched OUT payload. The native x86_64 control
module consumes this same input and independently enforces its actual DMA
buffer size; a broader request bound cannot broaden that storage. Legacy
enumeration still constructs its requests in Rust, as migration debt. This raw
input seam does not install a USB Host Call, grant endpoint possession or prove
asynchronous cancellation, reusable rings, class execution or native OUT-data
device compatibility. Those require the remaining stack work below.

`src/usb_base/control_payload` decodes the core's canonical U8 collections into
256 bytes of native-owned storage without allocation. It uses borrowed core
access rather than a USB-specific serializer; element identity, full canonical
validity, destination capacity and transfer geometry are independent checks.
The exact control Kind derives its port Types from `control-types.conduit`
during preparation. `control.conduit` calls it through ordinary checked topology.
The request carries setup and at most 256 OUT octets; its native decoder checks
the exact reviewed schema before reading fields, then checks transfer geometry.
The result is a closed variant: completed with actual count, shortness and
bounded IN octets, or stalled, provider-lost or unsupported. Kernel cancellation
remains cancellation control/terminal truth. Result encoding refuses impossible
counts, mismatched IN bytes and input data attached to an OUT completion.

Core borrowed record access and prepared canonical leaf/record/variant
composition support this boundary without a USB serializer. Composition checks
exact child Types, its prepared byte envelope and aggregate depth/node limits.
Preparation may allocate; these request/result hot paths use finite storage.
Definition and encoding do not yet install a selected native USB Host Call or
prove device execution, cancellation or DMA quiescence.

`src/usb_base/control_owner` binds an already-issued opaque Base possession to
the exact sealed Plan, canonical active Play and ordinary lowering. Register
and USB preparation share that verifier. Native attachment/slot/generation and
storage bounds belong to the trusted physical owner; requests cannot choose
them. Every submission reauthorizes its complete operation claim and receives
a move-only native ticket. One pending transfer holds pressure until acknowledged
physical quiescence. Revocation clears software possession but retains physical
pending state; a subsequent physical acknowledgement cannot publish a late
successful result. Tickets from another issuer, slot or attachment generation
cannot clear a genuine pending operation.
The shared single-call Back's kernel request IDs remain the correlation source;
replaying a completed request is refused rather than repeating a native effect.

`src/usb_base/control_factory` prepares the exact selected control operation
with the production kernel's shared single-call Back. The checked control plot
states the same 4096-byte canonical input/output envelopes as the Kind; the
256-octet transfer payload bound remains independent. Preparation rejects
substituted profiles, ports, call bounds, Base mechanisms, resources and grants.
A deterministic fixture checks and plans this source, issues explicit inert Base
possession, and runs dispatch and completion through the production kernel and
control owner. Two transfers preserve exact results, advance request identity and
reuse the bounded output buffer. Wrong Boot, missing dispatch resources/authority, forged request
tokens, duplicate completion and cancellation are refused. This is cooperative
kernel execution proof with scripted quiescence, not an installed native offer,
controller execution or a physical stop acknowledgement.

The quiescence acknowledgement is currently a private unsafe native-provider
boundary, exercised by cooperative fixtures. Its safety contract requires the
exact final status or acknowledged endpoint/controller stop. Software timeout,
cancellation and loss alone are insufficient. These fixtures do not demonstrate
real controller stop, DMA retention or dispatcher installation; the actual
native owner must enforce that lifetime independently before this is offered.

Native control completion retains a short Data Stage residue until the final
Status Stage succeeds; status success cannot replace the actual data count.
Duplicate stages, foreign slot/endpoint identities and malformed counts remain
distinct refusals. The legacy descriptor read overrequests 64 octets and still
requires exactly 18 returned descriptor octets. The x86_64 emulator proof now
requires an observed short transfer, followed by successful status and device
configuration. This demonstrates native stage handling, not plotted enumeration
or cancellation/quiescence proof.

The native control producer retains one finite cursor in each DMA slot instead
of reconstructing a presumed enumeration offset. One reservation holds pending
storage until the exact final Status Stage succeeds. Uncertain failure, timeout
or foreign completion poisons reuse; it does not claim endpoint stop. Slot and
attachment-epoch checks reject stale device descriptions. Only acknowledged
Disable Slot retirement releases the storage for new enumeration.

Control TDs remain contiguous within the fixed ring. A Link TRB returns to the
head and toggles producer/consumer ownership. Publication withholds the first
TRB until all stages are ready, and withholds the next free slot before exposing
the TD. This includes tail slots that were unused in a previous cycle. Link
publication follows complete preparation of the new head. These x86 coherent
DMA stores are controller invariants, not a portable coherence claim.

The architecture proof appliance scripts 64 short descriptor transfers through
this native primitive and requires exact ring geometry, cycle transitions and
bounded storage in `cargo xtask make conduitos usb-proof`. Its separate ring Sign
and receipt explicitly identify fixture protocol. That proof does not execute
USB class plots or establish physical-device compatibility. Deterministic tests
exercise 100,000 mixed reservations and 10,000 actual DMA publication cycles,
including controller look-ahead, pressure, stale reservation and uncertain
quiescence. The [xHCI specification](https://www.intel.com/content/dam/www/public/us/en/documents/technical-specifications/extensible-host-controler-interface-usb-xhci.pdf)
defines Link TRB and Cycle-bit behavior.

## Bounded endpoint receive on development code

`endpoint-read.conduit` invokes the class-neutral `machine/usb/endpoint-read`
leaf through the production Host Call boundary. Its result carries packed
`Bytes <=2048B`; the maximum canonical completed result is 2,516 bytes within
its selected 4 KiB call surface. The payload bound, representation extent and
call admission budget remain separate contracts.

The x86_64 native owner binds exact Boot/Plan/Play possession and device/endpoint
attachment generations. Configure Endpoint must succeed before Root advertises
readiness or issues possession. One Normal TRB owns the retained receive buffer;
uncertain completion retains DMA, and cancellation revokes possession before
acknowledged Disable Slot retirement. The Link cycle changes only with the final
ordinary TRB, preserving a controller waiting at the preceding cycle's Link.

```sh
cargo xtask make conduitos usb-proof --endpoint-read
```

This dedicated emulator appliance executes the checked endpoint plot with an
explicit Root-selected `usb-kbd` fixture. It verifies 128 eight-byte receives,
two complete ring-cycle transitions, the exact canonical byte transcript,
normal kernel drain and acknowledged native stop. Its retained receipt records
Boot, Source/checked plot, Plan/Play and attachment identities and explicitly
sets `class_acceptance` to false. Deterministic conformance separately exercises
all receive lengths through 2,048 bytes and sustained allocation-free reuse.
This proves the x86_64 raw endpoint path; it does not establish HID class plot
execution, the other architectures' controller paths or physical compatibility.

## Check the groundwork

```sh
cargo xtask make conduitos usb-plots-check
cargo xtask make conduitos usb-plots-check --cross
```

The first command checks/expands the source and executes deterministic wire and
register-possession fixtures. The second also type-checks the shared library for
x86_64, IA-32, AArch64, RISC-V64 and LoongArch64. The IA-32 check uses the installed
UEFI Rust target; it is not a BIOS image or product boot check. Both commands
require the checkout's pinned toolchain; cross checks require those Rust target
libraries to be installed. No target is silently skipped.

Tests cover endpoint identity, exact setup bytes, signed mouse edges, fractional
PCM packet cadence, ring-cycle reuse, bounds, stale Boot and revocation. Neither
command establishes USB controller execution, device playback, Ethernet traffic,
physical behavior, or stable acceptance.

## Remaining implementation

The native control owner binds an exact selected control Back, Boot, attachment,
capability possession and retained DMA slot before dispatch. Cancellation keeps
native storage until acknowledged Slot disable. An acknowledged normal transfer
returns the device to the composition root without resetting its cursor or
issuing a new grant.

The x86_64 proof appliance requires 64 short control transfers through the
checked `control.conduit` Source and production kernel, followed by normal closure.
The host reconstructs its exact Source, checked/expanded plot, Plan, fragment and
Play identities from the observed Boot/controller/attachment and verifies a
separate bounded transcript receipt. Its explicit cooperative proof grant does
not establish hostile-code confinement or authorize discovered devices for
ordinary product use. This raw-transfer fixture does not interpret USB classes.

The proof root admits 2048 additional local and 256 additional remote lifecycle
Sign items during preparation, then runs 64 complete input/output pairs and
normal closure through one kernel Play. Its versioned receipt binds those
finite bounds and requires at least four complete ring-cycle transitions.
The ordinary default profile retains its earlier finite Sign budget and exact
exhaustion refusal. Additional storage grants no controller authority and cannot
grow during Play. The separate native ring receipt remains fixture evidence;
neither receipt establishes checked class execution.

`device-probe.conduit` constructs the device-descriptor request, frames the
actual returned bytes and decodes the descriptor through ordinary expression
and selector Backs in the production kernel. The x86_64 proof requires 64
exchanges, acknowledges both observed transfer and decoded descriptor outputs,
and verifies exact Source/Plan/Play/attachment identities and ring reuse in a
separate receipt. Its preparation admits 4096 additional local and 512 remote
Sign items, fixed output buffers and one in-flight transfer. Consumed Unit
inputs release their value slots after Host Call completion even though their
canonical payload is empty; deterministic sustained execution verifies reuse.
The proof still uses legacy attachment setup and an explicit cooperative grant.
It is descriptor emulator evidence, with no class or physical compatibility claim.

The protocol calculations and register definition are not installed product
device offers. Register calls execute through the production kernel in a
cooperative mapping fixture. The following work remains under the owning issue:

- product machine/USB offers, native dispatcher and actual resource integration;
- a class-neutral bounded USB transfer base with distinct short/malformed,
  pressure, cancellation, provider-loss and stale-attachment outcomes;
- finite DMA/coherence/interrupt possession and lifecycle;
- shared controller discovery, enumeration, descriptors and alternate interfaces;
- control and reusable interrupt/bulk/isochronous transfer plots;
- keyboard transition validation/delivery and mouse semantic mapping;
- negotiated USB Audio Class PCM playback, pressure, underrun, drain and loss;
- CDC-ECM networking through the existing network semantic seam;
- CDC-ACM serial and mass-storage bulk-only transport with separate acceptance;
- architecture-specific mapping/coherence/interrupt leaves and real emulator
  execution of the same class plots on each supported product target.

The existing x86_64 Rust USB/HID stack remains the current product implementation
until replacement plots earn execution proof. Sharing arithmetic is useful
foundation; it does not establish driver parity.

For #4833's migration audit, the existing xHCI mapping, DMA/ring ownership,
interrupt acknowledgement and controller lifecycle are base/invariant work.
USB descriptor decoding, HID report interpretation, probe/class selection and
FTDI command/framing behavior are protocol work retained as temporary
implementation debt until equivalent plot execution is proven. This is a
responsibility classification, not a claim that current files already enforce
the final boundary. Do not delete the current path or maintain two permanent
protocol implementations. Acceptance must correlate exact source/checked plot,
selected base/resource, Plan/Play and attachment identities with a bounded
transaction transcript and final lifecycle outcome. Emulator execution and
physical/HIL compatibility remain separate proof classes.


## HID keyboard state (development)

`hid-keyboard-state.conduit` derives twenty fixed slots from validated,
normalized previous and current reports: modifier changes in bit order, sorted
releases, then sorted presses. Unchanged slots are explicit.

`hid-keyboard-lifecycle.conduit` retains previous-report state and drains each
batch through generic state, zip and merge Backs before pairing another command.
The command entry finishes explicitly. The received-frame entry connects the
checked decoder and normalization to that loop; invalid reports are observed
without replacing previous state. Generic ordered concatenation appends a
Source-authored finish command after the decoded report stream closes normally.
The state loop drains all pending transitions before completing; cancellation
remains a separate kernel outcome. Native device teardown proof is separate.

Deterministic kernel conformance covers 64 report generations and 1,274 ordered
transitions, held-output pressure, explicit finish, cancellation, and malformed
wire reports, with zero Play allocations and storage reserved beforehand.
Run it through `cargo xtask make conduitos usb-plots-check`. These tests establish
neither an installed class offer nor interrupt endpoint execution, physical
compatibility, or five-architecture USB emulator acceptance.

## HID boot selection (development)

`hid-boot-control.conduit` owns the HID boot-selection exchange over
`machine/usb/control`. It constructs an interface-scoped, no-data SET_PROTOCOL
request and requires an empty, non-short completion before reporting ready.
Malformed, stalled, provider-lost and unsupported results remain distinct.
Interface numbers are wire data; the selected attachment and transfer authority
come from the containing Plan and native owner.

The existing control proof kernel executes this Source with explicit fixture
grants. `cargo xtask make conduitos usb-plots-check` covers all 256 interface
numbers, inconsistent completions, preserved failures, and bounded kernel
execution without hidden retries or Play allocations.

`cargo xtask make conduitos usb-proof --endpoint-read` runs this checked
exchange against the actual emulated keyboard before the raw endpoint workload.
Its separate receipt correlates Source, checked plot, Plan, Play, attachment and
interface identities, the independently expected transaction digest, normal
close and quiescent release. The native owner has possession for one operation;
endpoint reads require their own subsequent admission. The retained endpoint
workload still crosses two complete ring cycles and acknowledges device stop.

Ordinary HID setup still uses the retained Rust path. The dedicated emulator
appliance establishes neither physical compatibility nor ordinary class offers
or class acceptance on the five product architectures.

## Protocol references

- [USB HID 1.11](https://www.usb.org/sites/default/files/documents/hid1_11.pdf),
  Appendix B boot reports and section 7.2 class requests.
- [USB Audio 1.0](https://www.usb.org/sites/default/files/audio10.pdf), endpoint
  frequency control and Type-I PCM streaming.
- [USB CDC specifications](https://www.usb.org/document-library/class-definitions-communication-devices-12),
  Ethernet Control Model packet-filter and communications interface contracts.

HID report interpretation consumes packed `Bytes <= 2048B`, matching the bounded
endpoint payload. The class plots require count/extent agreement and reject boot
reports larger than eight bytes before reading an octet. Keyboard reports require
eight bytes; mouse reports preserve three through eight actual bytes, including
uninterpreted extensions. Short reports remain distinct from malformed reports.
These are class bounds, independent of endpoint transfer admission.

`hid-endpoint.conduit` connects the exact endpoint-read Fore to keyboard and
mouse interpretation. Source constructs the eight-byte boot-report request,
checks count, extent and shortness, and preserves stalled, provider-loss,
unsupported and timeout observations separately. Only completed valid frames
reach the class decoder. Preparing these graphs publishes no physical Back;
execution requires a separately selected endpoint offer and actual possession.
Deterministic conformance checks both class compositions and their bounded
framing. Both class graphs also have the native emulator proofs below.

The dedicated keyboard Source appliance enters through
`cargo xtask make conduitos usb-proof --hid-endpoint`. It uses the same bounded
xHCI endpoint owner as the raw endpoint proof and checks 128 alternating fixture
presses/releases across two ring cycle transitions. The retained receipt checks
exact Source/Plan/Play and attachment identities, the canonical class output
transcript, normal closure and acknowledged stop. The production arena is sealed
before Play, so the entire native transfer, drain and stop run refuses allocations.
This command requires its own
proof image with a 32 MiB preparation arena. Deterministic preparation with the
production allocator peaks at 17,781,120 live bytes and releases all retained
storage on retirement; the arena also admits allocation geometry and the native
Root's storage. The endpoint call still admits at most 2,048 payload bytes on a
4 KiB call surface. A retained x86_64 emulator run completes all 128 reports with
the arena sealed. The appliance retains legacy attachment setup and an explicit
Root fixture grant. This development proof does not establish an ordinary class
offer, physical compatibility or five-architecture acceptance.

The mouse counterpart enters through
`cargo xtask make conduitos usb-proof --hid-mouse`. Its separate proof profile
selects the existing mouse Source graph and injects 128 alternating button and
relative-motion reports. The independent expectation follows
[QEMU's HID report implementation](https://github.com/qemu/qemu/blob/v10.2.1/hw/input/hid.c):
the three-octet boot prefix is decoded, and the fourth wheel octet remains
uninterpreted wire truth. Mouse and keyboard receipts have separate paths and
require their exact Boot profile, Plan, class transcript, sealed arena and
acknowledged stop. A retained x86_64 mouse run completes all 128 reports across
two ring cycle transitions with matching class outputs and the arena sealed
through normal closure and acknowledged stop. The same appliance limitations
apply: legacy attachment setup, explicit fixture authority, and no ordinary
class offer, physical compatibility or five-architecture acceptance.
