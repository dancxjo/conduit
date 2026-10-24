# Environmental observation Body

The construction specifications are Conduitese, consumed by the existing Host
and Body checkers:

- `profiles/environment.body.conduit` admits the persistent sensor part alone.
- `profiles/environment-browser.body.conduit` retains the same Body and part
  identities and adds a self-joining browser recorder. The recorder is optional;
  its invitation name requests a binding and does not mint authority.
- The sensor Host selects the registered Pico USB implementation and declares
  finite GPIO, clock, local retention and access-point resource needs.
- `profiles/environment-esp32.body.conduit` and
  `profiles/environment-esp32-browser.body.conduit` request the same compositions
  on the HW-463 ESP-WROOM-32, with its exact package descriptor. They select only
  the registered kernel/signal Back; peripheral needs remain requests. This does
  not select or build the climate diagnostic image.
- The recorder Host selects the registered browser Face, WebSocket and IndexedDB
  implementations, with a finite durable-storage budget.

Check these exact files without building firmware, deploying Spores or starting
Play:

```sh
cargo xtask prove environment specifications
```

This focused entrance uses the real RP2040/ESP32/browser make packages and shared
Host/Body checkers. It checks all four compositions and canonical Source round trips.
It does not install missing providers or establish runtime admission. The
ordinary general entrance remains `cargo xtask make body check <body-file>`.

## Workload boundary

Sampling and retention policy belong to ordinary Plots. The Host specifications
declare finite resource needs; they do not make GPIO, time or networking offers
appear. The current RP2040 make package registers USB only. Its existing firmware
AP appliance is physical implementation material, not an installed logger Back.
The attached DHT11 uses GP2, 3V3 and ground; this physical selection stays outside
the protocol Plot.

The generic `machine/pulse/read` Fore and its prepared canonical codecs now
exist in `mechanisms/io/pulse-capture`. The RP2040 realization uses PIO/DMA,
with Source-owned DHT11 geometry; it has finite physical diagnostic evidence.
That library and Fore are not yet published as an installed Host offer.

`dht11-read.conduit` composes the capture request, pulse Fore, Source frame
projection and DHT11 decoder. Timeouts, provider loss and refused geometry leave
through separate typed ports. `runtime` assembles existing expression, selector
and Host Call Backs under the production kernel; it contains no scheduler.
A deterministic transcript proof plans and lowers the complete graph, checks
bounded fixed-image metadata, and executes valid, corrupt, timeout, provider-loss
and geometry-refusal cases. Play allocates nothing and performs no implicit retry.
The pulse transcript provider is retained through `ReadyPulseBase`, the shared
Base registry and ordinary resource/authority planning, then consumed into the
selected `PulseHostCall`. Missing authority prevents planning. Replacement
provider generations fail binding before effects. The source and observation
fixture Backs remain test instrumentation, and the provider performs no physical
effects.

The optional pulse `host-call` feature publishes only a retained ready provider
and binds it to the exact sealed
fragment, ordinary lowering, active Play, authority and opaque Base capability.
Cancellation drops the provider future before revoking unfinished possession.
The RP2040 driver implements that boundary and compiles for `thumbv6m-none-eabi`.
Its cancellation guard releases the line and disables the state machine; the
HAL DMA future aborts on drop. Physical cancellation and radio coexistence still
need hardware proof. The retained 32-frame diagnostic evidence does not use this admission
path or the production kernel.

Focused verification:

```sh
cargo xtask prove environment pulse
cargo xtask prove environment pulse-target
cargo xtask prove environment kernel
cargo xtask prove environment kernel-target
```

The pulse proof includes allocation-free completion/cancellation, replay refusal,
malformed completion and exact binding tests. The fixed-image computation
profile explicitly admits bounded canonical expression and selector metadata;
the default generator still refuses such text configuration. Metadata bounds do
not admit evaluator work or heap consumption. Installing native offers and
admitting the physical kernel composition remain before autonomous sampling.

The host test currently measures a 2,551,985-byte peak across Source checking,
expansion, planning and lowering; this is host allocation evidence, not an MCU
heap estimate. The Pico must consume prepared material. The existing assigned
Plan encoder emits 44,859 bytes for this graph, with a declared 15,148-byte
execution-state projection. The ordinary tiny-host decoder retains its 128-byte Text ceiling. The explicit
computation profile now admits these records without allocating, only when
node, key, exact length and digest match the trusted resident image's inventory.
Per-record and total encoded-text ceilings remain independent. Make derives
that inventory after checking the exact expression or selector contracts; the
device must not derive it from an incoming assignment. This validates computation
metadata, not complete kernel tables, evaluator work, native authority or storage.
Those projection numbers exclude prepared evaluator storage and do not establish
Pico RAM admission.

The requested manifestation is Observation → Face → seven-segment Mask → Show →
TM1637 protocol → selected electrical Base. The existing universal Face grammar
carries first-class quantities, freshness and sensor status. The Mask owns
selection, units, rounding, negative-value fitting, overflow and stale policy.
Its Show has four segment patterns, four independent decimal-point requests and
a colon request. A hardware profile maps those requests to physically connected
bits and must expose shared controls and unrepresentable combinations.

The [seven-segment Mask](../../plots/seven-segment-mask/README.md) now has an
ordinary Mask-role Plot, checked Show and Look Types, a compact SegmentPattern
Form, deterministic formatting and wiring-fit logic. Its selected Look requests
Fahrenheit without changing the Face quantities. The TM1637 command packets are
authored in the device-protocol Plot. Installed MCU capture/time/AP/display Backs,
autonomous kernel Play and overnight proof remain implementation work. The
TM1637 is not wired or physically verified.

The browser may contribute the logging store across an admitted Line. Device
sampling and manifestation must continue without it. Local retention is bounded;
connection loss, storage pressure and missing history remain explicit. A browser
page's durable records do not imply that its Host/Boot is currently present.
Neither construction file currently installs or executes that logging flow.

`runtime::assigned` now loads the local fixed-source projection into ordinary
kernel node, Cord, routing and Host Call tables. It validates the exact resident
structural inventory before table preparation; The explicit schema-5 kernel projection carries Cord single-value envelopes
and pressure policies on the wire. The legacy schema-4 encoder and tiny-host
admission remain unchanged; the kernel loader refuses that incomplete projection. It validates startup dependencies and prepares Backs in the assigned
order. Record access shares the assigned format's bounds-checked reader.

The host proof executes the decoded tables for all five sensor outcomes and
checks equality with ordinary lowering. A replacement Boot produces a fresh Plan
while retaining the resident structural layout. Loading allocates nothing; the
full runtime compiles for RP2040 through `kernel-target`. This local profile does
not yet install the Pico loader or provide constrained native capability binding.
Remote endpoints and resource Cords remain outside this loader profile. Complete
physical RAM/work admission, autonomous sampling and browser logging are pending.

The explicit schema-6 native projection adds the selected cooperative Base,
provider generation, capability, Kind revision, execution profile, authority
grant and local resource binding to each native Host Call. Its current profile
supports one unprotected resource and one authority grant per cooperative native
call; unsupported bindings refuse encoding. Schemas 4 and 5 remain unchanged.

`ReadyPulseBase::bind_assigned` compares those selected facts with actual retained
ownership and the current opaque capability table. It also checks full Plan and
fragment identities against the assigned projection, ActivePlay, exact typed
ports and Host Call bounds. The shared rich/assigned scope check preserves the
same current Host/Boot/Plan/Play and resource-generation rules. A serialized
binding cannot issue a capability or establish permission to use a pin.

The host kernel proof now uses this constrained binding for all five pulse
outcomes. Twenty re-digested native-field substitutions are refused before any
provider effect. This supplies native operation binding for a constrained loader;
actual Pico Boot/Body integration, complete physical memory admission, and a
running logger image are still pending.

A separate attended Pico kernel bench is available through
`cargo xtask prove dht11 build-kernel`. It constructs a fresh local Boot/Plan and
uses the actual retained PIO pulse provider. Physical bring-up has reached
ordinary sealing and native binding, then stopped during Back preparation.
The recovery image is built; physical sensor execution through this kernel is
still unproven. This 192 KiB bench profile does not admit the production 96 KiB
Host/Body composition, autonomous logging or radio coexistence.

Core now hashes fragment canonical bytes incrementally, reusing scratch storage
between placements and contracts. The retained complete sensor Plan and fragment
identities remain byte-for-byte unchanged. This reduces preparation copying; it
does not claim a universal heap bound for arbitrary Plans.

The `face` package now contributes the semantic observations to the universal
Body Face, using the existing Temperature and Ratio contracts. It preserves
units and precision, supports temperature-only sensors, and keeps freshness
separate from the latest sensor outcome. The supplied contribution basis passes
through unchanged. Run `cargo xtask prove environment face` for the contribution
through the existing Fahrenheit Mask. This proves the semantic path on the host;
the contribution builder allocates Face objects and still needs admission and
connection to the recurring MCU workload.

`dht11-observe.conduit` now provides the semantic observation entry point. It
composes the Source-owned pulse/frame decoder with `device/dht11-observation`,
a pure Back that applies the checked quantity Forms. Decoder coordinates remain
internal representation material; the public `Dht11Result` carries Temperature
and Ratio quantities. Capture timeout, provider failure and refused geometry
still leave through their distinct ports. Frame/checksum failures retain their
original typed payloads.

The additional host kernel proof plans and lowers all 14 nodes and 13 Cords,
binds the ordinary retained pulse owner, and executes the production scheduler
for positive, negative, checksum, short-frame, timeout, provider-failure and
geometry-refusal transcripts. The emitted quantities feed the environmental
Face contribution and the Fahrenheit Mask. Play and representation refusals
allocate nothing. Face construction occurs after that Play allocation boundary.

A complete canonical observation result occupies 2,274 bytes in this profile:
each Quantity includes its full Unit descriptor. The existing planner's per-Cord
limits give the observation output a 4 KiB envelope while pulse/selector Cords
retain 1 KiB. Prepared leaf buffers reserve their exact fixed Quantity envelope;
the host proof reserves two large value slots and thirty small ones. This is
host conformance and an RP2040 compile check, not physical heap/stack admission
of the expanded sensor workload. The existing 13-node raw-capture bench remains
separate, with its previous exact Plan identities preserved.

## Finite sampling cadence

`profiles/sampling.conduit` composes `time/tick`, a typed tick-to-trigger
projection and the existing DHT11 observation Plot. Its default 960 one-minute
samples cover sixteen hours and close normally. Electrical timeouts, provider
failure, refused geometry and decoded sensor failures remain distinct.

```sh
cargo xtask prove environment cadence
```

The portable `conduit_time::TickBack` is shared with the standard Host. Its
counted mode retains one pre-admitted wait and emits sequence values through
the existing kernel canonical boundary, with constant Back storage. The new
`time/tick-trigger` Back validates a Tick and emits Empty; it owns no clock or
cadence. Tests cover zero/count bounds, all 960 ticks, output pressure, malformed
ticks and cancellation. Planning checks zero, short and overnight Source profiles.

The Pico bench physically completed four three-second readings through one
allocation-sealed Play before a USB reader attached. A late reader observed
explicit stale Shows. This proves autonomous short sampling and optional
inspection, not a full overnight history, an installed Body workload, radio
coexistence or a connected browser store. The latest observation per outcome is
retained; the historical store and AP/browser composition are the next integration.
The physical TM1637 attachment awaits connection and an attended display test.

The user has selected 3.3 V operation for the TM1637 module: VCC (even if
labelled “5V”) → Pico 3V3 OUT (physical 36), GND → common ground (physical 38),
CLK → GP3 (physical 5), DIO → GP4 (physical 6). This bypasses the level shifter;
all signal pull-ups must reference that 3.3 V supply. DHT11 DATA remains on GP2.
Disconnect USB before wiring. These are selected physical attachment facts,
not confirmation of wiring or an installed display offer.

The portable `mechanisms/io/tm1637` transport now supplies finite open-drain
wire sequencing for Source-encoded packets, including byte ACKs, STOP on NACK,
cancellation and best-effort line release after provider loss. Run
`cargo xtask prove environment display-wire` for deterministic wire traces.
This component has no installed GPIO Host Call binding or physical display proof.


The optional HTTP AP bench now builds through `cargo xtask prove dht11 build-http`.
It reuses the same Source sensor decoder and first-class observation Back inside
the production kernel, with 4096 samples at 15-second intervals. Its prepared environmental Face
retains the latest semantic quantities; the HTTP Mask expresses them in Fahrenheit
and relative humidity at
`http://192.168.4.1/` on the open `Conduit-Climate` AP. This separate bench has
bounded DHCP/HTTP buffers and prepares network storage before Play seals the
allocator. It does not install new offers in the production Host construction
profile, retain overnight history or realize the optional browser recorder Line.
See the Pico proof README for exact bounds and proof limitations.

## Authored appliance publication

`profiles/appliance.conduit` connects the existing finite sampling Plot to four
real typed publication kinds. The optional `construction` feature of the
runtime crate checks and expands this Source without constructing a fake
provider or advertising physical availability. Its resulting graph contains
no fixture kinds. It is construction material, not an installed Host/Body or
an executable Plan; actual ready offers, authority and resource admission are
still required before Play.

`PublicationBack` reserves its validator and two canonical buffers before Play.
It retains one exact typed value per outcome, exposes no uncommitted value,
refuses an exhausted event budget, and preserves the distinction between
capture timeout, provider failure, geometry refusal and decoded sensor results.
The ordinary kernel's read-only driver view supplies committed publications to
the Face owner. No external mutable observer cell or second scheduler is needed.

```sh
cargo xtask prove environment publication
```

This checks Source expansion and publication commit, refusal, closure and
cancellation behavior, including zero allocation in Play. Firmware installation,
Body membership and the browser recorder remain separate integration work.

The host proof also measures preparation heap separately from Play. On the
64-bit test host, the 15-node appliance used a 277,176-byte peak during ordinary
planning and 35,247 bytes during lowering, excluding the already-live Source
and Host descriptions. Independent clones peaked at 121,391 bytes for the
expanded graph, 64,879 bytes for the Host advertisement, and 141,115 bytes for
the Plan. These are diagnostic observations, not MCU admission bounds; clone
capacities and pointer widths differ. They favor preparing the rich Plan on a
host and admitting its exact bounded native projection on the MCU. Such an
installation still requires current initialized offers, Boot identity, authority
and resident layout checks; these measurements do not establish that path.


`bodies/environment/make` now also exposes `PreparedNativeAppliance::prepare`.
It consumes the checked appliance Source, supplied Host advertisement, chosen
Bases, authority and explicit artifact ceilings. It uses ordinary planning and
lowering, produces the native assignment, and retains the rich Plan on the
planning host. Make creates no Ready provider. `compile_default()` preserves
the count and cadence authored in `profiles/appliance.conduit`.

```sh
cargo xtask prove environment native-construction
```

This transcript proof covers the default Source through native production
kernel-table admission without capture effects, missing authority, a stale
Boot and inadequate projection capacity. The complete publication kernel proof
also runs through the native assignment and selected pulse-owner path: all
seven sensor scenarios allocate nothing during Play. Its native assignment is
46,121 bytes and declares 20,722 bytes of projection state; these exclude Back
preparation, kernel storage, Face, network and Root memory. The trusted resident
records now pin numeric count/cadence as well as graph and computation records,
so a re-digested assignment cannot replace Source configuration. Authentication,
real Body/Wake integration, target memory admission and physical installation
remain necessary before this construction material becomes an appliance.

The prepared Face owner also exposes a fixed seven-segment Show snapshot through
the existing Fahrenheit Mask and a caller-selected hardware profile. HTTP and
seven-segment output retain the same Face identity and revision; physical I/O
happens outside the Face lock. Freshness loss renders the Mask's unavailable
Show. The snapshot path allocates nothing. It does not yet install or acknowledge
a TM1637 Host Call.


## Network setup construction

`profiles/network.conduit` is a finite three-gear control Plot using the existing
`network/credentials`, `network/join` and `network/attachment-sign` contracts.
It carries no SSID, password, radio identity or transport facts. Make checks and
expands it with the ordinary network bootstrap catalogue.

The HTTP library now supplies a private, bounded form decoder and one-slot
`NetworkSetupMailbox` for the credentials Host Call. The caller supplies the
selected implementation's credential limits. This uses the existing nonempty
credential contract; open-network joining is not admitted by that contract.
Duplicate/unknown fields, malformed escapes, invalid UTF-8 SSIDs, capacity
pressure and exhausted submission budgets refuse before configuration work.
A submission becomes consumable only after its exact local acknowledgement;
the HTTP caller must flush the response first. Small output buffers preserve it;
successful transfer consumes it once. Cancellation and Drop zeroize the owned
buffers. The input and mailbox have no Debug, Clone or serialization API.
The HTTP request and admitted output buffers have separate caller-owned lifetimes.

```sh
cargo xtask prove environment network
```

This establishes Source checking and zero-allocation private input transfer,
not an installed credentials offer, HTTP route, or radio transition.
The mailbox grants no authority. Root must plan and admit the existing network
operations against its actual initialized radio and current Boot, bound both
association and address configuration, and restore the AP on failure or link
loss. Those physical transition and browser interactions remain integration work.


The portable `conduit-net` contract now supplies finite station-attempt
mechanics for an already admitted Host Call. Root provides the physical service,
one deadline spanning AP withdrawal, association and address configuration, and
a separate restoration deadline. Every failed or expired phase makes one cleanup
attempt; readiness is reported only after that succeeds. Cleanup failure remains
an explicit unavailable outcome. The mechanics select no credentials, authority,
implementation, retry policy or Plan. Their deterministic tests model future
cancellation and stage ordering; they are not physical timeout evidence.

The existing `pico-w-signal` network/join Host Call now uses the same station
mechanics with one 30-second deadline shared by association and DHCP. It keeps
its existing selected request, attachment identity checks and error meanings.
This integrates the shared deadline into a real target execution path, but does
not yet install the environmental AP-switch service. That service still needs
its physical AP withdrawal/restoration operations and link-loss supervision.


The active hardware target is now the user's HW-463 ESP-WROOM-32 board. The
selected attachment is DHT11 DATA → GPIO27 (D27), TM1637 CLK → GPIO18 (D18),
and TM1637 DIO → GPIO19 (D19), with both modules supplied from the development
board's labelled 3V3 and GND pins. The expansion board's VCC rail is not assumed
to be 3.3 V. These are new physical binding requests, not reused Pico pin numbers
or proof of initialized ESP32 providers. The sensor Source, quantities, Face,
Mask and network request contracts remain portable. The old Pico firmware and
physical receipts remain evidence for that target only.

The HTTP input boundary now also owns a fixed 1024-byte, single-request buffer.
It waits for the complete declared form body across partial TCP reads, bounds
headers to 512 bytes, refuses ambiguous length or transfer framing, secret-bearing
query routes and pipelining, and erases its raw bytes on consumption, refusal or
Drop. Root must still bound the connection lifetime. This is request framing for
the setup page, not an installed HTTP route or permission to change radio mode.

The setup Look renders a bounded POST form with the selected implementation's
credential limits. A `PendingNetworkResponse` exclusively borrows the mailbox
while its accepted response is written. Dropping it on a failed write, timeout
or cancellation cancels the submission; `response_flushed` releases it exactly
once to the independently admitted credentials call. Rendering and cancellation
proofs allocate nothing. Root still needs to install the route and invoke the
actual admitted transition service; rendering a page does not install that service.

The connected ESP32 ROM reports revision 3.0, a 40 MHz crystal and 4 MB flash.
This identifies the current hardware without changing the separate reviewed
HW-463 sample descriptor or claiming initialized sensor/radio providers.

## Recorder representation boundary

See [Recorder construction](docs/recorder-construction.md) for the current
checker, readiness, storage and Face APIs, their proof scope and remaining gaps.

`PreparedObservationQuantityForms` packs a validated `Dht11Observation` using
the two quantity Forms already authored in `dht11.conduit`: two bytes for
Temperature followed by two for humidity Ratio. Unpacking reconstructs the
canonical observation with first-class quantities. Packing is exact; inexact
precision and coordinate overflow refuse instead of applying display rounding.
Both paths use storage reserved during preparation and allocate nothing in Play.
The selected recording session must bind the exact semantic contract and both
Form identities; these four bytes alone carry no identity, sequence, clock,
status, provenance, integrity or transport meaning. Sensor failures need their
own explicit record variants and must not replay the last valid reading as new.

The supported `cargo xtask prove environment kernel` suite includes these codec
proofs. They do not establish durable storage or a recorder Line. The existing
IndexedDB implementation permits 64 physical records per application, so the
4096-slot recorder Host request cannot be satisfied as 4096 independent entries.
Recorder integration still needs finite chunks within those physical bounds,
exact sample and provenance admission, transactional commit, deduplication and
explicit gaps across reconnects. No larger storage offer is claimed here.

The installed browser storage boundary is narrower than the IndexedDB primitive:
`resource/publish-json-snapshot` admits immutable JSON snapshots, at most 4096
content bytes, under an exact Resource generation, residence and authority. The
browser adapter checks the pending Host, Boot, Play, placement and request before
calling `publishBytes`. `writeBytes` exists as a storage primitive but is not a
selected recorder Host Call; calling it from a recorder page would bypass the
existing execution boundary.

The recorder must therefore publish finite sealed chunks through that existing
snapshot Fore. Its semantic records retain quantities and explicit sensor
outcomes; the selected quantity Forms are representation material inside the
JSON envelope. The envelope must bind both Form identities, the exact semantic
contract and actual producer Host/Boot/Plan/Play provenance. Boot-relative times
must remain distinct from browser receipt time and wall-clock estimates.

The following are implementation obligations, not installed capabilities:

- Admit chunk content within 4096 bytes including metadata and JSON encoding;
  reserve sample buffers and encoding storage before Play.
- Keep buffered samples distinct from committed samples. A failed, cancelled or
  quota-refused publication cannot advance the committed cursor or acknowledge
  durable acceptance to the producer.
- Publish each selected generation once. A partial flush consumes a chunk slot;
  do not hide updates, retry, eviction or a growing generation count in the adapter.
- Budget at most the actual admitted physical record count, currently bounded
  above by 64 per application. Sixty-four samples per chunk can retain 4096
  samples only when the planned flush pattern and other application records fit.
- Reconnect under new actual presence and authority while reading retained
  generations by their exact references. Retained records do not restore a Boot,
  membership, a live Line or a grant.
- Validate finite chunk contents before use; distinguish identical replay,
  conflicting sequence reuse, out-of-order delivery, sequence gaps, producer
  reboot and reversed boot-relative time. Never fill missing history with the
  most recent valid temperature.

The recorder Host requests 64 physical records and 1 MiB per application. This
is a construction request; exact snapshot generations, authority and the complete
flush pattern still require admission before live recording.
The compact quantity codec alone supplies neither chunk commit nor recovery.

`runtime::recording::PreparedRecordingChunk` now prepares one bounded immutable
JSON snapshot content value with up to 64 committed producer events. The status
projection and its byte discriminants are authored in `profiles/recording.conduit`
and generated through the ordinary Type/Form compiler. Available events pack
canonical observations through the existing quantity Forms. Failed events carry
no quantities, so the last valid temperature cannot silently become a new sample.
This projects sensor status rather than retaining the producer's full raw capture
or failure payload.

Preparation binds caller-supplied Host, Boot, Plan and Play identities, the exact
observation Type digest and all three Form identities. Identity strings use
reversible UTF-8 hex inside JSON. Each record contains one common committed event
sequence, producer-boot milliseconds, the status Form and the quantity Forms.
Independent publication endpoint counters are not interchangeable with that
common sequence. Identity bounds and complete metadata extent refuse before Play
if a full chunk could exceed the selected 4096-byte content envelope.

Append detects identical replay within this chunk, conflicting sequence reuse,
out-of-order input, reversed boot time and gaps between accepted entries. The
first entry makes no claim about history before recording began. Full chunks
refuse further new samples. Seal creates stable JSON bytes and forbids mutation;
append and seal allocate nothing after preparation. No API labels a chunk as
committed, chooses a Resource generation, writes IndexedDB, retries or retires
content. Cross-chunk recovery, actual publication completion,
finite generation admission and the producer Line remain integration work.

The ESP32 display now has an attended physical diagnostic under
`targets/esp32/proof/tm1637`. The selected GPIO18/19 wiring was exercised at a
100 microsecond half-period with open-drain release and internal pull-ups. All
segments, `1234` and the `67.6F` fixture were acknowledged; the user confirmed
both digits and the point after 7. This establishes that physical display path,
not an installed seven-segment Mask/Host Call or a real temperature display.

`PreparedSourceFrameDecoder` now turns a complete frame into canonical results
by executing the actual checked `dht11-decode` expression and existing observation
quantity Forms. It contains no native checksum, sign, range or decimal decoder.
All storage is prepared once; frame conversion and the resulting Face/Mask path
allocate nothing. Source document, checked Plot and expanded Plot identities are
generated from that same compilation, without inventing Plan/Play or Body/Wake.
The physical initial frame `[106,0,40,4,151]` is unavailable(checksum); subsequent
`[53,0,20,2,75]` frames yield 20.2 degrees Celsius and 53 percent relative humidity.
The Fahrenheit Look renders those quantities as `68.4F` in the host proof. The
ESP32 cross-check passes; the currently flashed fixture still shows test patterns
until the live Source/Face/display application is integrated.

`PreparedDisplayPackets` now executes the checked `tm1637-packets` expression.
Its prepared composers encode register, brightness and enable inputs; its output
is a fixed owned three-packet batch. Command bytes and brightness refusal remain
in Source. Native code only validates finite packet geometry and exposes borrowed
wire slices for the existing electrical transport. The end-to-end host proof now
covers captured bytes → Source validation → quantities → Face → Fahrenheit Mask
→ verified digit/dot mapping → Source-encoded TM1637 packets, with no recurring
allocation. The Root still has to invoke the exact admitted physical Host Call;
this pure encoder is not permission to drive pins or an installed Body.

`PreparedRecordingReader` now validates the exact canonical chunk representation
against a caller-supplied expected Host/Boot/Plan/Play, observation Type and all
three Form identities. It refuses malformed or oversized content, unknown status
bits, failure entries with quantity payloads, duplicate/reversed sequences and
reversed producer time. Refusal preserves the previously loaded chunk. Reading
reconstitutes canonical Temperature and relative-humidity quantities, with local
gaps explicit, without allocation after preparation. It grants no storage
authority, authenticates no producer and makes no cross-chunk continuity claim.
The existing Resource read completion must establish the selected snapshot's
authority and generation before supplying these bytes.

Run the focused chunk encoding/recovery proof with
`cargo xtask prove environment recording`. It includes full 64-entry recovery,
refusal without partial replacement, context/Form substitution and zero-allocation
reconstruction of exact canonical quantities.

The recorder Host request now uses the concrete IndexedDB ceiling of 64 physical
records per application, rather than treating 4096 sample events as 4096 storage
slots. Each immutable chunk holds at most 64 events. Partial flushes and any
manifest consume the same finite record pool; neither eviction nor additional
index slots are implicit. The 1 MiB aggregate request remains a requested ceiling,
not evidence that browser storage is available or authorized. Actual Resource
generations and the whole publication closure still require admission.

The ESP32 platform adapter in `targets/esp32/network-switch` now implements the
portable station/AP execution traits against the real SDK and distinct network
stacks. It preserves one absolute join deadline, invalidates old address state,
and requires Root-owned service retirement/restoration acknowledgement. Pending
SDK association gets cancellation cleanup; radio mode alone never establishes
service readiness. `cargo xtask prove environment network-esp32` cross-checks
this machinery. The page/control-Play connection, initialized offers, lifecycle
admission and physical station/fallback/link-loss proof remain outstanding; the
current flashed AP bench is unchanged.

### Station-side local name

The requested local address is derived from the current Body's actual friendly
name, using its first-name component: `<body-first-name>.local`. This belongs to
the device's **station/client attachment** on the joined LAN. It is not an AP
SSID policy, DHCP hostname substitution, a minted Body identity or an AP-only
DNS alias. The corresponding HTTP URL is `http://<body-first-name>.local/`.

Root must prepare a valid finite DNS label from the current Body biography,
retain the chosen label in Face, and bind the mDNS responder to the exact current
Host/Boot/station attachment and actual assigned address. A name collision must
be detected and resolved through an explicit bounded name-selection policy;
silently claiming an existing name is not success. Link loss, address/name change,
Lull or station withdrawal retires that responder. Graceful retirement should
send the protocol goodbye when the attachment still permits it; loss of a link
cannot guarantee remote-cache invalidation. No placeholder name is advertised
before a Body name exists. An MCU or browser recorder joining/leaving does not
change the Body's identity.

The current ESP32 AP bench has no installed Body biography and does not yet
advertise this name. Responder installation, multicast resource admission,
collision handling and real client-network resolution remain required. The
Embassy `mdns` flag resolves names as a client; it does not by itself install a
host responder. The bounded no-alloc responder in
[edge-mdns](https://docs.rs/edge-mdns/0.8.0/edge_mdns/) is under evaluation;
no dependency or availability claim is implied by that investigation.

The HTTP service-control mailbox now admits one control handle and a finite
command count. Acknowledgements are correlated with their actual command;
cancelled starts can be superseded, and old acknowledgements cannot complete a
new stop. The ESP32 socket task reports readiness only after DHCP binding and
both HTTP listeners, and retirement only after dropping all socket/request
futures. Restart and failure clear the local AP-ready observation. Startup now
awaits that acknowledgement instead of sleeping for an assumed startup interval.
`cargo xtask prove environment http` checks the HTTP/input/control contracts;
ESP32 and Pico cross-checks pass. These source/CPU proofs do not yet establish a
physical stop/restore cycle or connect the page to an admitted control Play.

`PreparedNativeAppliance::body_plot_plan` preserves the exact authored Source
identity and ordinary Plan as a Body workset partition. `propose_to_body` passes
that partition and explicitly supplied Mask topologies to the existing
`BodyLifecycleSession`. The owner checks the complete workset and current
Host/Boot membership before retaining a Wake and sealed Body Plan. Wrong
worksets, stale Boots and repeated proposals refuse without changing evidence.
The returned realization has no started Play. Root still owes durable retention,
MCU memory/resource admission and an attributable start result before the normal
`started` transition. Multi-Plot callers compose `body_plot_plan` with their other
partitions rather than discarding them. The public native-construction proof now
checks this CPU boundary; its membership/provider fixtures are not physical
admission evidence.

A remaining start boundary is explicit: the selected native-operation binders
currently verify standalone `bind_active_play` identities for a per-Plot Plan,
whereas Body execution binds a `BodyPlayIdentity` over the sealed Body Plan and
Wake. They are different identities. Native installation needs an explicit,
Root-authorized Body-to-fragment execution binding; relabelling either identity
or removing checks without replacement is not a valid integration.

`NativeBodyBinding` now retains the separately canonical Body and fragment Play
identities for the exact proposed partition. Root supplies both prospective
identities from its own issuers; in particular a Body-local counter must not be
reused as the Host/Boot-wide fragment counter. Confirmation requires the exact
started Body Play retained by the ordinary owner. Unstarted/different Plays,
stale membership, Lull and explicit revocation refuse. Current-owner checks
allocate nothing, and the ordinary native binder and its capability scope keep
the fragment Play identity unchanged.

This host-side correlation object is not an exclusive execution claim, MCU
resource admission, a serialized start receipt or replacement for capability
revocation. Root must supply its current retained owner, persist the correlation,
reserve real target resources, and connect actual start/terminal receipts before
using it in physical installation. CPU fixtures exercise those lifecycle gates
and the existing native binder; they do not prove a physical Body Play.

`make::PreparedNetworkSetup` prepares the checked three-gear network setup Plot
against supplied current Host offers, selected Bases and explicit grants. Its
ordinary Plan/lowering and Body workset material start no Play. The
`cargo xtask prove environment native-construction` proof now connects this
Source through the real private mailbox, selected join owner and fixed kernel.
Only the radio and Root authority/presence are CPU fixtures. Missing credentials
authority refuses planning; unacknowledged page input refuses execution before
join; acknowledged input is consumed once and reaches the attachment-sign call.
Diagnostic allocation measurements are hosted observations, not MCU memory
admission. Compiler/planner preparation remains host-side. Firmware Root
installation, current Body lifecycle, private page routing and physical network
transitions remain unproved by this test.

Publication Backs for one local producer now require a shared
`PublicationSequence`, prepared with a finite total event allowance. Each Back
retains its own admitted channel limit, while snapshots carry one `u64` order
across readings, capture timeouts, provider failures and geometry refusals.
Staging reserves capacity; only the kernel's commit assigns the next number.
Cancellation, replacement of an uncommitted step and dropping a staged Back
return that reservation. The sequence is local to a Play and does not supply
Host/Boot provenance or sample time. This closes the independent-channel-counter
gap; durable publication, recorder transport and physical endurance remain.

`recording::PreparedRecordingInput` validates each publication's exact channel
Type, projects the authored observation result into the recording disposition,
and appends the committed common sequence to a chunk. All eight sensor outcomes
round-trip through the existing chunk reader without execution allocation;
wrong-channel Types and zero sequences refuse without adding records. The caller
must supply actual producer sample time and retain the selected producer context.
This component owns no transport, durable storage or acknowledgement.

The shared selected-operation binders now accept exact zero-or-one input/output
shapes, permitting a credential source or an attachment sink without inventing
an unused Plot port. Rich and ordinary native-assigned projections agree for all
four shapes and reject extra ports or a substituted trusted shape. Existing
Host Call bounds, Base possession, current identities, resources and authority
checks remain required. This enables endpoint binding; it does not initialize
a private credential Base or install the physical network control Play.

`NetworkCredentialsHostCall` retains a private `CredentialSource` behind the
selected credential contract and independent credential-reader authority subject.
It checks the exact current Plan/Play, Base/resource binding and opaque possession
before reading the source once. Wrong requests, replay, revoked possession and
short output buffers cannot read the mailbox. The real private mailbox is tested
through this owner with acknowledged and unacknowledged page submissions, with
no execution allocation. Root must own erasure of the caller's admitted output
buffer after credential custody ends. Native assigned credential binding, actual
initialized private Base projection and physical page/control installation remain.

`ReadyCredentialBase` now retains the initialized private source and independently
owned native identity across planning, advertises its bounded resource through
the ordinary Base registry, and consumes that ownership into the selected read.
Stale-Boot and duplicate projections refuse without changing the advertisement.
The checked network Play proof now dispatches both credential and join requests
through their selected owners; its dispatcher no longer reads the mailbox
directly. Root authority/presence and radio remain explicit CPU fixtures. Actual
MCU installation and native assigned credential binding remain outstanding.

The retained credential Base now also binds the ordinary native assigned-plan
projection. Native implementation/artifact/Base/resource identities come from
that owner, not assignment bytes. CPU fixtures exercise both rich and assigned
binding with actual private mailbox submissions, revocation, replay and a
substituted artifact. Assigned binding and execution allocate nothing. The ESP32
network adapter cross-check passes; this is compilation evidence, not firmware
installation or a physical network transition.

`PreparedNetworkSetup::prepare_native` now emits the ordinary native assignment
and Make-owned resident layout for the checked control Plot. The network Play
proof exercises both rich and native-assigned realization: both retained owners
bind the assignment and the fixed kernel tables are prepared from it without
allocation. Foreign expected Boot and missing trusted layout refuse. Acknowledged
credentials reach one join and attachment sign; unacknowledged input reaches no
radio effect. This is CPU integration evidence, with explicit Root/radio fixtures;
authenticated MCU delivery, Body admission and physical switching remain.

The shared Pico/ESP32 AP socket loop now uses `http::exchange` for bounded
request framing and response writes. Root may install a `NetworkSetupRoute`
alongside its admitted private source/control Play; the current bench passes no
route and still returns 404 for setup. Installed setup routes render the prepared
page, accept one private form and release it only after the entire response is
written and flushed. Short writes are handled; write/flush failure or dropping a
pending flush cancels and erases the submission. Raw request buffers are erased
on drop and responses never reflect credentials. CPU transport fixtures cover
these paths with zero execution allocation. Firmware Root route installation and
physical joining remain outstanding; no new firmware has been flashed.

`cargo xtask prove environment root-esp32` builds a separate ESP32 native Root
preparation mode. It consumes the real GPIO27 provider into `ReadyPulseBase`,
retains the initialized controller and both interface stacks, and projects that
retained pulse offer under the observed eFuse Host and radio-TRNG Boot. It does
not read the sensor, execute an assignment, issue authority, install a Body or
start HTTP setup services. Serial output explicitly retains false Body/Plan
installation flags and a null image-attestation field; the implementation
artifact label is symbolic, not an image hash. A brief physical capture observed
two complete offers on the attached ESP32: one Ready pulse Base, one resource and
one capability under a fresh Boot. No sensor read or Body start occurred. The
retained bench restoration flash completed. The user confirmed temperature on
the physical display, but the USB connector broke before serial/HTTP rechecking.

The Root preparation image accepts an exact `offer` line on UART0 (GPIO1 TX,
GPIO3 RX). It returns the current retained offer with a 64-byte command buffer,
a five-second request deadline and a two-second response deadline. Unknown or
oversized commands are refused; assignment installation is not implemented.
CPU framing tests and ESP32 build/lint checks cover this exchange; physical
verification is outstanding. Host-spec-driven construction, assignment admission
and full physical installation remain incomplete.


`PreparedRecordingSnapshot` prepares a fixed sample count against one exact
bounded snapshot reference. Its JSON extent is known before samples arrive;
the chosen content profile, item count and byte extent must match. Partial
sealing refuses while retaining buffered samples, since a shorter flush cannot
reuse a reference admitted for a full chunk. Failures occupy the same finite
record width as available quantities. Committed publications can enter through
the existing typed recording projection, retaining the producer sequence and
producer Boot time. Encoding and sealing allocate nothing during execution.
The returned reference bytes are an expected Host Call output, not a durable
receipt. Actual selected snapshot invocation/completion, committed-cursor
advancement and recorder membership/Line integration remain outstanding.

`RecordingSnapshotBack` crosses the ordinary snapshot Host Call protocol rather
than exposing a public “mark durable” method. Before requesting storage it checks
the complete fixed-size chunk against the selected producer context, semantic
Type, Forms and sample count. A completed call must return the exact selected
reference. Only the kernel's committed completion Step publishes the sequence
range to its read-only observer. Output pressure, failure, denial, cancellation,
wrong references and corrupt input cannot advance that observer. One Back owns
one publication; retries and additional generations are not implicit.

`cargo xtask prove environment recording` exercises this Back in the production
fixed kernel, plus cancellation between staging and commit and blocked output.
Execution allocates nothing. Numeric placement and external storage completions
in this proof are explicit CPU fixtures: browser installation, actual storage
acknowledgement, recorder membership/Line and physical endurance are still
required for a deployed recorder appliance.


The replacement ESP32's annunciator test was visually reported as `11:11`,
with no decimal point. Its bench hardware profile therefore declares a colon
and no decimal points. The explicitly selected Fahrenheit Look uses that colon
as the fractional marker (`71:2F`); it retains tenths instead of reducing to
whole degrees. This is Mask policy, not a false decimal-point wiring claim.
`TimedSevenSegmentMask` owns the three-second temperature/RH page alternation;
its prepared lifetime is finite (24 hours in this attended bench). After that
lifetime it renders an explicit unavailable Show. Acquisition still updates one
shared quantity Face every fifteen seconds. The HTTP Mask and display Mask
consume that same revision; neither owns a second observation state.

The replacement ESP32 revision 3.1/4 MB has been flashed with this bench. Serial
capture shows acknowledged alternating temperature and `H` pages with the same
Face identity between sensor updates. AP startup is reported; the new image has
not yet been checked over HTTP. Page-to-station control remains uninstalled.

`NetworkSetupChannel` gives prepared socket writers and one non-Clone private
reader shared custody of the existing finite mailbox. A socket retains its
asynchronous lock through the response flush; the selected source uses a bounded
try-lock and reports Busy separately from missing/unacknowledged input. Dropping
a stalled response erases the submission before releasing custody. The actual
stream exchange and concurrent writer/source fixtures exercise this boundary
without execution allocation. Input collection may precede SDK/Plan preparation;
collecting it grants no radio authority. The MCU route remains disabled until
its Root service is installed.

Root may synchronously borrow the acknowledged input with
`prepare_acknowledged` to prepare SDK-owned configuration before planning and
Play. The borrowed bytes cannot escape the callback; the original submission
remains for the selected credentials Host Call, including after preparation
refusal. This path neither allocates nor authorizes a radio operation itself;
Root must admit the callback's preparation memory separately. The HTTP proof
checks flush/custody ordering, refusal without consumption, exact later input,
zero mailbox allocation and compile-time refusal of escaping secret references.
Actual control Play installation remains unfinished.

The ESP32 Root preparation image now initializes and retains that actual private
source alongside the GPIO Base, under the observed Host and Boot. Its writer is
retained by Root. The earlier physical receipt covers initialized-source availability,
not credential presence, a network join offer, an installed control Play or Body
birth. The management UART exclusively owns its output after initialization and
starts offers on a fresh line, avoiding truncated startup logging in the protocol.

The updated Root preparation image starts the fixed DHCP/two-listener HTTP
service with that writer. `/network` explicitly collects preparation input only;
its acknowledgement says that no join has started. Reading routes return 503
instead of constructing a fictional observation Face. An `offer` request
try-locks the mailbox, prepares and retains the SDK station configuration once,
and reports `network_configuration` as waiting, busy, refused or prepared without
revealing submitted bytes. The selected credential source retains the same input
for its later admitted Host Call. The service uses the existing five-second
connection deadline, including mailbox custody, and five-second startup deadline.
The [physical Root input receipt](evidence/2026-10-23-esp32-root-input/README.md)
now records actual AP/DHCP/HTTP service and a private fixture submission followed
by three prepared offers under one Boot. The temperature/RH bench was restored
afterward; actual network control remains uninstalled.

`PreparedRecordingHistory` recovers an admitted finite number of chunks for one
exact producer Host/Boot/Plan/Play. It retains one chunk and a recovery summary,
exposes missing sequences across chunk boundaries, and refuses overlap, replay,
reversed producer clocks, foreign context and excess chunks without replacing
prior recovery. Available samples retain the observation's first-class quantities;
failure samples retain their disposition. It neither creates a storage manifest
nor claims durable acknowledgement. Each generation still requires independent
snapshot authorization and restoration through the existing browser executor.
The recording proof recovers 64 chunks (4,096 samples) without execution
allocations; this is deterministic CPU recovery, not an overnight browser run.

`RecordingRecoveryBack` receives the existing snapshot-read Fore's JSON through
the production kernel. Two preallocated workspaces keep candidate content
separate from committed recovery. Only `step_committed` exposes the next chunk
to a Mask; discarded Steps, failed reads, corrupt input and cancellation cannot
advance the retained history. Exhausting the admitted chunk count is a storage
refusal, distinct from malformed content and cancellation. The recording proof
checks this with the real kernel and explicit CPU read-completion fixtures,
including zero execution allocations. Browser installation, actual storage
acknowledgement and recorder membership/Line are still separate requirements.

The ESP32 Root issuer reserves Play identities from one checked monotonic
Host/Boot sequence before issuing possession. A computable canonical Play
identity alone is insufficient: it must match Root's retained reservation.
Invalid fragment, Host or Boot requests preserve the counter and current
reservation; exhaustion refuses rather than wrapping. Reservation neither starts
Play nor retires an earlier provider. The setup installer uses this allocator,
and its one acknowledged credential/join budget is not renewed by another
reservation. CPU conformance covers current and superseded reservations and
execution after an earlier unused reservation. Environmental Body installation
and producer-to-recorder admission remain separate work.

`ReadySoftwareBacks` initializes and retains the actual interpreter, selector,
observation and publication objects before projecting their software offers.
Ordinary selection transfers each object once, only after matching its retained
Source, configuration, Front, implementation, artifact and published generation.
Refusal preserves the object; selected objects are omitted from new advertisements.
The timer and pulse provider remain independently initialized and authorized.
The appliance conformance proof now uses these retained software owners rather
than constructing software offers from Source descriptors and reconstructing
unrelated drivers after planning. All outcome publications still share one
finite committed sequence, and the production kernel remains the scheduler.

This is a rich preparation profile, not an MCU memory-admission claim. The
current 64-bit CPU measurement observes 232,595 bytes of peak heap for software
preparation, 121,391 for a graph copy, and 281,438 for ordinary planning. Its
native assignment encoding is 46,121 bytes with 20,722 declared projection bytes;
those are distinct from the complete execution closure. The ESP32 Root's 64 KiB
arena therefore has no admission proof for this rich path. MCU installation must
use the separately bounded native assignment path and prove its complete live
closure before Play. No new physical Host, Body, sensor or browser installation
is claimed by these CPU measurements.

`PreparedAssignedAppliance` initializes the actual software drivers directly
from the bounded native assignment and independently pinned resident roles,
layout and computation inventory. It checks those before preparing the timer
value, codecs and publication endpoints. It retains no rich Source, Plan or
offer catalog. Pulse and wait Host Calls still require independently owned
native providers, current possession, complete resource admission and a real
Body start. This constructor neither issues authority nor claims installation.

The publication proof now runs all seven sensor outcomes through both rich and
assigned preparation after dropping the rich preparation objects on the assigned
path. Wrong Host/Boot, colliding or mislabelled roles, altered computations and
insufficient wait storage refuse before software allocation or provider effects.
Both paths use the production kernel without execution allocations. On the
64-bit CPU, assignment-based software preparation originally peaked at 150,803 new heap
bytes and retained 118,776; preallocated value storage, assignment bytes, resident
metadata and native owners are outside that measurement. The complete MCU
closure remains unproven. The ESP32 native-root feature compiles this appliance
profile, but no physical appliance installation is claimed.

Host Wi-Fi HIL setup uses `cargo xtask setup environment-host-wifi` in a
foreground host terminal. It displays the exact proposed command, then requires
host sudo authentication to install a grant for the existing climate-test
connection UUID and one Wi-Fi interface. The installer validates the
`Conduit-Climate` SSID, refuses Ethernet and unrelated sudoers files, and checks
its snippet with `visudo` before installing it. It reads no network credentials
and does not activate a connection during installation. The scoped file is
`/etc/sudoers.d/conduit-climate-test-<user>`; removing that file revokes this grant.

`cargo xtask prove environment host-wifi` subsequently activates that exact
profile on that interface with noninteractive sudo. Activation failure remains a
failure; the command never edits profiles or supplies a password. This is Linux
operator tooling, separate from MCU authority and Body membership. Opening this
ESP32's serial port with both DTR and RTS asserted allowed three Root offers to
be read in the previously observed Boot; deasserting them sequentially during
open had caused the earlier resets. An observer timeout does not retire or reset
the device's independently owned setup operation.

Make now produces an owned `NativeSoftwareRecipe` from its checked ordinary Plan
and generated image. It carries resident roles, exact record counts, normalized
layout/configuration fingerprints and the exact computation inventory. It can
outlive the rich Make state; it contains no Host, Boot or Plan identity fields.
Root must authenticate and pin that recipe separately from incoming assignments.
CPU proof verifies equal recipes across fresh Boots while the assignments and
Plans change, and different recipes when authored cadence changes. The default
960-minute workload is preserved from Source, rather than duplicated in Rust.

Assignment preparation now initializes the largest temporary evaluator first,
before retaining observation/publication codecs. Execution still follows the
admitted tables. The same 64-bit CPU measurement reports 119,307 new peak heap
bytes (118,776 retained), excluding the separately preallocated value store,
assignment bytes, resident metadata and native owners. This reduces preparation
peak; it still does not establish the complete MCU memory closure.

The appliance now exposes `value_storage_bytes()` and `prepare_value_storage()`
for its canonical kernel values. This reuses the kernel's preallocated store:
two 4 KiB slots and seven 1 KiB slots (15,360 payload bytes), with slot metadata
counted separately by the architecture-specific admission estimate. Callers
must admit that cost before construction, alongside the other live storage.
Both rich and constrained CPU paths use this production profile. All seven
sensor outcomes also drain the authored 960-sample, one-minute workload with
zero execution allocations. These accelerated clock completions establish
kernel/storage behavior; they do not establish physical overnight operation,
browser persistence, or the complete ESP32 memory closure. The profile also
passes the ESP32 native-root release cross-check.

`NativeSoftwareRecipe::write_resident_rust` emits a standalone firmware module
whose `resident()` borrows static layout and computation requirement arrays.
Provision that module through the trusted firmware build independently of
received assignments; compiling received metadata into expectations would not
establish trust. The emitted recipe is identical across fresh Boots, while
the actual assignments and Plans differ. Its generated Rust was compiled
against the production core, kernel and appliance runtime. This enables resident
acceptance data without heap-owned recipe vectors; it does not yet provision
the physical Root, initialize a provider or complete appliance installation.

`PreparedRecordingBatch` prepares the complete finite recording workload from
caller-supplied immutable resource references and explicit generation/content
ceilings. It initializes every chunk encoder, publication Back and commit
observer before Play. Full chunks precede a final shorter chunk; references
must have exact extents and unique identity/version pairs. The 960-sample
workload needs 15 generations. Partial flushes need separate admission, rather
than silently changing this closure. The constructor neither mints references
nor installs resource authority, routes values or performs storage effects.
Root still admits the full heap/work/envelope closure and selected storage.
CPU proof encodes and recovers all 960 samples without execution allocations;
all observers remain uncommitted because that proof invokes no durable store.

`cargo xtask prove environment publication-32` repeats publication/construction
and kernel proofs as 32-bit Linux executables, including all seven 960-sample
scenarios. It requires the i686 Rust target and 32-bit C linker/development
files. The retained [measurement](evidence/2026-10-23-software-i686/measurement.json)
reports 92,679 peak new heap bytes (92,320 retained) for assigned software,
plus separately existing 46,121 assignment bytes, 15,504 canonical value backing
bytes and a 6,760-byte prepared object. The software alone exceeds the current
64 KiB ESP32 Root arena. These CPU measurements establish that the present
preparation is not admissible there; they do not establish ESP32 ABI sizes,
allocator overhead or stack high-water. Releasing the assignment buffer before
software initialization requires a distinct validated-admission phase and
independently pinned resident computation data, rather than treating received
program bytes as resident authority.

Native initialization now has two phases. `ValidatedAssignedAppliance::admit`
checks current assignment tables, roles, exact computation requirements and
Source cadence without allocating; its owned result borrows no input buffer.
Root can release the assignment before `initialize` checks independently
provisioned `ResidentComputation` text and initializes the software. Missing,
extra, duplicate, wrong-key or altered computations refuse before software
allocation. Make retains those texts independently and emits static firmware
text as well as the exact acceptance digests. No current identity or authority
is inferred from that resident data. The convenience `prepare` entrance still
retains its borrowed assignment through initialization.

The 32-bit proof now drops the received assignment before initialization and
passes all seven overnight scenarios. Software's own 92,679-byte heap peak is
unchanged; the separated phases eliminate assignment-buffer overlap when
firmware actually uses the emitted static texts. CPU fixtures retain owned
recipe text separately. Neither this lifetime proof nor the ESP32 cross-check
establishes the complete physical MCU memory closure or a Body start.

The constrained appliance now reserves one port ordinal per node, matching
every input and output in the checked authored graph. Its kernel conformance
compares the exact cords/fuel with ordinary lowering, verifies unused broad
input slots are empty and executes all seven overnight outcomes. Incoming
ports outside this finite profile refuse; changing the authored Fore requires
renewing admission. On i686 the scheduler object falls from 38,256 bytes for
the broad sixteen-port profile to 10,584 bytes; the prepared object falls from
6,760 to 4,508 bytes. See the retained
[measurement](evidence/2026-10-23-unary-kernel/measurement.json). These are object
sizes, not stack high-water or complete MCU memory-closure evidence. Software
heap preparation remains 92,679 peak bytes, above the current Root arena.

Per-computation [32-bit preparation measurements](evidence/2026-10-23-computation-memory/measurement.json)
identify the authored DHT decoder as the largest software component: 75,917
peak and 40,042 retained requested heap bytes when constructed separately.
The other six computations retain between 3,012 and 4,177 bytes each. Individual
peaks are not additive, and these numbers exclude other live storage and
allocator overhead. This identifies the next memory-admission work; it does
not justify a larger Root arena or claim a physical installation.

`PreparedRecordingFace` publishes bounded recovery evidence through the ordinary
Face grammar: recovered chunk/sample counts, first/last sequence, known missing
sequences within that range, and distinct content/capacity/cancellation refusals.
A refusal preserves the last recovered range; duplicates preserve revision, and
invalid, reversed or exhausted updates preserve the prior Face. Preparation
reserves all update storage. CPU tests project the recovery Back's committed
summary into this Face without further allocations. The caller supplies the
actual contribution basis and only committed recovery state. This Face neither
acknowledges a new durable write nor treats historical samples as fresh sensor
readings; quantity observations remain in the existing observation Face/history.
The browser Mask and real recorder Host installation remain separate work.

The HTTP Mask's `recording_response` formats that sealed recovery Face into a
finite JSON or HTML Show, preserving its exact identity and revision. It keeps
no recorder state, rejects foreign/inconsistent Faces and allocates nothing
during formatting. Root must still provide the actual Face and install its
selected route; this formatter alone makes no endpoint or recorder Host live.
The observation page no longer asserts a hard-coded sampling cadence or AP-only
connectivity; neither follows from its semantic observation Face.

`install_recovery_kind` makes `environment/recover-recording` available to the
ordinary Source checker and expansion catalogs without creating a provider.
`ReadyRecordingRecovery` separately owns an initialized recovery Back, advertises
it in the actual recorder Host/Boot and transfers it once after exact ordinary
selection. Source identities, semantic Fore, generation, bounds and realization
all remain checked; refusals preserve the retained object. The historical
producer context is independently supplied and can differ from the recorder's
current Boot. CPU proof checks real Source/Plan selection, altered selections,
retirement from fresh offers and allocation-free committed recovery. This is
software readiness, not storage authority, a Line, browser installation or a
Body/Play start. Root still admits and binds those independently.

The Root diagnostic now enables the allocator's internal peak counters and
reports separate Root/SDK current and peak usage. The instrumented image was
flashed to the replacement ESP32; five offers remained in one Boot with no
installed control Play or dispatch. Root peaked at 17,908 bytes (9,684 current),
SDK at 33,716 (33,408 current), during startup and idle AP service. The arenas
remain 64/120 KiB. These are observed allocator estimates, not join/fallback
limits or a justification for shrinking the SDK arena. Retained
[physical evidence](evidence/2026-10-23-physical-heap-highwater/proof.json)
identifies the exact flashed ELF. The corresponding 32-bit appliance proof
accounts separately for 2,880 sign-backing bytes and 15,504 value-backing bytes;
its object sizes do not establish ESP32 stack or complete memory admission.

The ordinary prepared expression evaluator now boxes its large member/sequence
selector variants during preparation, so unused selector-buffer capacity is
not repeated in every arithmetic node. On the same 32-bit workload this reduces
software peak/retained requested heap by 1,584 bytes, to 91,095/90,736 bytes.
The tradeoff is 34 additional preparation allocations; one small frame
projection grows by four requested bytes. Execution remains allocation-free
across all seven overnight scenarios, and focused expression conformance,
strict Clippy and the ESP32 cross-check pass. See the retained
[measurement](evidence/2026-10-23-expression-selector-storage/measurement.json).
No authored decoder or semantic contract changes, no firmware reflash and no
complete MCU admission are claimed from this private storage improvement.

### Browser read correlation

`browser::PreparedRecordingBrowserReads` prepares the existing browser snapshot
operation for every selected batch read. Its borrowed effect uses the existing
`conduit.browser/resource-effect@1` adapter schema and the original kernel
request sequence. Completion checks the full pending request and Host/Boot/Play/
placement scope before validating the immutable record and returning its content
to that request. Corrupt records become failed Host Calls. Replayed or foreign
completions cannot replace a pending read. History changes only after kernel
steps commit the recovery sink. Preparation allocates; begin and completion do
not grow storage.

`cargo xtask prove environment recording-storage` includes the production bridge
with explicit CPU storage fixtures, forged scope/request and corruption cases,
and zero-allocation execution measurement. The existing `recording-browser`
proof separately exercises actual IndexedDB persistence. These proofs are not yet
a live browser Body executing kernel-issued reads; Root still must bind the
actual store, current Body lifecycle and admitted memory/work closure.

The [browser/kernel read receipt](evidence/2026-10-23-browser-kernel-reads/measurement.json)
records this CPU boundary and the remaining live-recorder obligations.

### Original kernel reads through IndexedDB

`cargo xtask prove environment recording-browser` now also holds the production
15-generation recovery kernel pending while pinned Chromium publishes the
records, reloads the page and services its exact prepared browser read effects.
Returned scope and record bytes complete the original requests in reverse order;
only subsequent kernel steps publish the recovered 960-sample history. Equal
node-local request sequences from different placements cannot be interchanged.
Completion and kernel execution allocate nothing. See the
[retained receipt and transport artifacts](evidence/2026-10-23-kernel-indexeddb/measurement.json).

This combines actual storage I/O with production kernel correlation. Host
authority and Body start remain fixtures, and files carry the proof transport
between the native kernel and Chromium. It does not establish the real browser
recorder's Body/Line lifecycle, durable Root catalog-seed custody, MCU climate
memory closure or a physical overnight deployment.

## Native Host admission proof

The ESP32 Root preparation image answers `CONDUIT_BODY_ADVERTISE_V1` on its
bounded UART management carrier using the existing Body ambient challenge and
proof documents. Its Boot-local Ed25519 key comes from hardware TRNG with no
zero-entropy fallback. The actual current native advertisement and public key
are observation material: the owner must authorize the expected device/key
before the ordinary AdmissionManager verifies and consumes a challenge.

The UART carrier admits an 8 KiB advertisement, a 1 KiB challenge, five seconds
to receive it and two seconds per response. The original Pico USB carrier keeps
its own 4 KiB limit. Neither a returned signature nor an admission credential
installs an environmental Body, issues effect authority or starts a Play.
The responder validates current Host/Boot/offer generation; the Body authority
checks expiry in its own clock domain and rejects replay. CPU conformance uses
the production responder and ordinary Body verifier. The supported physical entrance is `cargo xtask prove environment body-admission-esp32`.
It passes on the attended HW-463 board; retained evidence is in
`evidence/2026-10-23-physical-body-admission/`. This establishes Host proof and
host-side membership, while owner-to-Root installation and Play remain unfinished.

`cargo xtask prove environment body-construction-esp32` joins the physical
admission and current-offer construction proofs. It retains a host-side Body
proposal in AwaitingPlan and its biography, with no Play or MCU installation.
It checks that admitted native facts still match Root before proposing; stale
scope refuses. The current proposal has no admitted Mask topology because Root
has no ready display/browser Mask owner. The proof exits after snapshotting;
this does not replace a persistent owner or its normal restart reconciliation.

The Root timer issuer can now validate a bounded assigned selection against its
actual retained timer before returning local possession. Rich and bounded paths
share capability preparation; the existing native binder checks attachment,
configuration and opaque authority. A refused selection preserves the issuance
budget and timer. CPU controls cover both record forms; this path is not yet
wired into physical Body installation or Play.

The pulse owner now supports a retained assigned transfer. It prepares native
binding and the canonical codec before taking GPIO ownership, so refusal leaves
the pin with Root. Root's bounded pulse issuer uses that same preflight and its
own reserved Play. Pulse and timer issuance both compare expected Source cadence
with admission's retained cadence; neither starts a Body or dispatches hardware.
The complete native kernel and authenticated installation handshake still need
to be admitted before physical appliance Play.

Retained software and assignment now also transfer through a boxed destination,
preserving both slots on refusal and moving existing drivers/selectors/value
backing on success. The ESP32 diagnostic adds `kernel-prepare` after assignment
staging: it prepares the ordinary dormant scheduler and sign log using
Make-emitted sign limits and an explicit additional-memory guard. This is not
an installation/start command and does not dispatch native operations. A
persistent authenticated owner, actual native possession and attributed Body
start remain required before any appliance Step. The physical entrance `cargo xtask prove environment kernel-prepare-esp32` now
passes on the attended ESP32. Evidence in
`evidence/2026-10-23-physical-kernel-preparation/` records 93,252 retained and
97,220 peak bytes in the 114,688-byte Root heap. This remains a dormant
preparation diagnostic; native/control admission and Body start are unfinished.

Root now has a separate provisioned owner-control verifier. It authenticates a
domain-separated transcript binding the operation, monotonic request sequence,
current Host/Boot/offer generation, full Body/Wake/Body Plan/Body Play and
Plot Plan/fragment/fragment Play identities, and assignment seal. Strict Ed25519
verification precedes sequence consumption; malformed, foreign, stale and replayed
requests preserve the counter. No provisioned owner key leaves this control
disabled. Incoming request metadata cannot provision a key.

This is an authentication prerequisite, not installation authority. The bounded
UART probe admits at most 2 KiB of JSON plus a 64-byte signature, with a five-second
receive deadline and two-second responses. It reports authentication separately
from application. `Reserve` correlates signed intent with actual admitted tables
and retains the scope. `Prepare` admits the same sealed transfer and dormant
native/kernel preparation; start/release/cancel remain `operation-not-installed`.
The build reads only an explicitly provisioned public key. Private-key custody,
the persistent ordinary Body owner, native installation and acknowledged start
remain unfinished. CPU controls run in `cargo xtask prove environment
native-construction`; the ESP32 image builds, but this probe has not been flashed
or physically verified.

The staged assignment receipt now includes a SHA-256 seal of the exact accepted
transfer. An authenticated reservation must match this retained seal and Root's
next full fragment Play identity, as well as the assignment's current Host, Boot,
Plan and fragment. Refusal preserves Root's reservation counter and native
providers. The accepted scope retains distinct Body/Wake/Body Plan/Body Play
identities alongside the actual reserved fragment Play; it is not a Body start
record. Root admits 4 KiB of remaining preparation/response capacity before
reservation and prevents diagnostic kernel preparation from consuming an
owner-reserved assignment. Native capability issuance, provider transfer and
attributed installation remain separate unfinished steps.

Signed `Prepare` now checks every retained reservation identity and returns an
opaque permit bound to the complete reservation. The owner then resends the exact
sealed assignment through a bounded 8 KiB transfer (two-second ready response,
five-second receive deadline). Root jointly preflights its actual retained pulse
and timer owners, Source cadence, selected operations and locally reserved Play
before spending either issuance budget or transferring either provider. It drops
record staging before preparing the ordinary dormant scheduler. If kernel
preparation refuses after native transfer, Root retains those native owners and
reports possession separately from kernel preparation; it does not pretend to
roll ownership back or start execution.

CPU controls exercise both assignment representations, each wrong provider role,
reservation identity substitution and successful dormant ownership transfer. The
ESP32 build passes. Physical native preparation, the persistent Body owner and
attributed start/release remain unproved; the diagnostic kernel image currently
on the board has not been replaced by this new control path.

`cargo xtask prove environment owner-prepare-esp32` provisions a private Unix
owner key under the host data directory (`conduit/environment-owner`), builds Root
with only its public key, saves an exact image receipt, and flashes the attended
ESP32. It then performs ordinary Host admission, captures current offers, stages
the exact assignment, and retains a real host-side Body owner and prospective
`NativeBodyBinding` across signed inspection, reservation and preparation. The
private seed remains outside the repository and firmware. Its directory/file
permissions are 0700/0600; mismatched public material, permissive seed files and
symlinks refuse rather than silently replacing keys.

This command is a finite attended preparation proof. The owner exits after its
final receipt; it does not establish durable owner restart, attributed Body
start, execution release or overnight sampling. Hardware success remains subject
to the actual retained native/kernel memory closure, with failures retained in
the capture rather than replaced by a fixture.

The first signed physical run authenticated and reserved the real scope, then
bound both native providers. Kernel preparation refused its old 27,660-byte
additional guard with 24,852 bytes free; the negative receipt is retained in
`evidence/2026-10-23-physical-native-capacity-refusal/`. No Body start occurred.
The assembly path now explicitly retires its transfer destination box before
allocating selectors and the scheduler. Admission uses the maximum of those two
stages plus coexisting sign backing, retaining the same 8 KiB control headroom.
The subsequent hardware run must establish the complete native/kernel closure.
Root startup also has a bounded 25-second read-only readiness poll before the
one-shot ambient admission challenge; signed mutations are not retried.

The subsequent full `owner-prepare-esp32` command passed physically; evidence is
in `evidence/2026-10-23-physical-native-preparation/`. The revised 23,692-byte
additional bound retained the same 8 KiB control headroom. Actual Root use was
100,524 of 114,688 bytes, peaking at 105,064, with both native Host Calls and the
ordinary dormant scheduler retained. The real Body owner stayed alive through
the exchange, then the finite proof process exited. Attributed start/release,
execution/service memory closure, Face publication and physical overnight
recording remain unfinished. The flashed device is prepared and unstarted.

The native execution bridge now separates bounded parameter extraction, provider
invocation and correlated kernel completion. A pending timer or sensor future
borrows its provider, leaving the ordinary kernel free to advance other Steps.
The target and CPU conformance use the same storage assembly. CPU controls cover
960 logical samples, concurrent pending-clock/reading publication, cancellation,
late completions, pressure, provider failure and revoked possession, without Play
allocation. See `evidence/2026-10-23-native-dispatch/` for the exact proof scope.
At that CPU-proof stage, worker/task admission and signed start/release were
unfinished; the later physical custody result is recorded below. No physical
reading is claimed by that bridge proof.

The target worker now owns finite pulse/timer request queues and a two-slot reply
queue around the ordinary kernel. It advances Steps while native futures remain
pending, wakes on completion, and observes actual committed publications. CPU
conformance covers 960 logical samples, concurrent publication, native failure,
revocation, cancellation and fenced abandonment with no Play allocation. Kernel
and native ownership transfer to one boxed context without cloning; Root guards
that context separately while retaining 8 KiB control headroom. See
`evidence/2026-10-23-native-worker/` for exact scope and measurements. That
CPU proof predates the physical task admission recorded below; signed
start/release remain unfinished.

Root now registers a concrete Embassy worker during signed preparation. The task
owns the boxed kernel, both native providers and the finite request/reply queues,
then acknowledges `ready-held` from the executor itself. It waits for a separate
command before polling execution. The finite owner proof requires zero Steps,
zero publications and zero native operations in flight, with both actual native
lifecycles still issued. Registration or readiness failure refuses preparation;
a readiness timeout requests cancellation rather than releasing execution.

The task admits a fixed 4096-byte projection of committed canonical publication
bytes; it grants neither logging authority nor Body start. Its status reports
actual capability inspections and preserves terminal resources. ESP32 compilation,
host lint and the portable native-construction proof pass. Physical task custody,
attributed Body start/release and Face/Mask service wiring remain to be proved.

The requested Raspberry Pi recorder/presenter, MCU construction role and symmetric
Linux/ConduitOS/browser handoff requirements are retained in
[SECOND-HOST.md](SECOND-HOST.md). Exact board identification and physical proof
remain required.

The full supported preparation command subsequently passed on the ESP32. The
real task acknowledged held custody with a 4,640-byte context and a 3,536-byte
held future, both providers issued and idle, and zero Steps or publications.
Peak Root use was 109,704 of 114,688 bytes. See
`evidence/2026-10-23-native-task-ready/` for the source-bound physical receipt.
The finite owner process still exits after preparation. Attributed start,
persistent ownership, release and actual climate sampling remain unfinished.

The owner-control acceptance now has explicit held-start and finite-run modes:

```sh
cargo xtask prove environment owner-start-held-esp32
cargo xtask prove environment owner-run-esp32
```

The admitted Boot key signs the exact owner command and outcome. The host verifies
that acknowledgement before `BodyLifecycleSession::started` and confirmation of
its `NativeBodyBinding`. Release is a distinct signed command. Cancellation is
acknowledged only after actual provider-future retirement, native revocation and
kernel cancellation; unknown cleanup does not manufacture Lull. Signed inspection
also binds a bounded committed publication's sequence, channel, size and digest.
The host projects its first-class Temperature/Ratio values through the ordinary
environment contribution and Body Face APIs. These UART captures are diagnostic
transport and proof artifacts, not an admitted application Line or recording store.

Physical start-held, release, cancellation of a pending native timer and ordinary
Lull passed on the corrected image; see `evidence/2026-10-23-owner-start-cleanup/`.
The first reading window was shorter than the checked one-minute cadence. The
revised window follows that cadence without changing Source. Its longer run hit
malformed UART ingress near the first expected sample; no readings are claimed.
Protocol 2 adds bounded 64-byte ingress credits so cooperative work cannot leave
an unpaced sender filling the FIFO. See `evidence/2026-10-23-owner-ingress/`.
Physical observation/Face acceptance, autonomous owner-loss fencing, the resident
service, application Lines, network transitions and durable overnight recording
remain unfinished. The finite proof keeps the owner alive only through its run.

The protocol-2 physical run subsequently passed: three real DHT observations at
the checked one-minute cadence became three validated ordinary Body Faces,
retaining their full first-class Temperature and Ratio quantities. The owner
verified each committed snapshot's signed scope/sequence/channel/length/digest,
then verified quiescence and recorded Lull. Final native counts were submitted
[3, 4], completed [3, 3], with 103 Steps, both capabilities revoked and zero
operations in flight. See `evidence/2026-10-23-native-observations/`. The board is
quiesced after this finite proof; this is not a resident or overnight recorder.

The next physical owner run also completed authenticated post-Lull retirement.
After three checked-cadence observations and verified quiescence, Root recovered
the actual revoked, idle GPIO/timer providers and dropped terminal kernel storage.
The Boot-signed Retired acknowledgement binds the separate exact-scope command.
Final Root heap use was 10,172 bytes (105,164-byte run peak), and the worker reports
zero execution-context bytes with no prepared kernel or live possession. Evidence
is retained in `evidence/2026-10-23-owner-retirement/`. The executor task remains
one-use; this does not yet rearm a new Play, advance resource generations or
establish a resident recorder. The board is retired and inactive after the proof.

The same retired native Boot then passed the negative network fixture through
`cargo xtask prove environment network-fallback`: HTTP acknowledged private
fixture credentials, the installed Root setup kernel attempted one join without
UART polling, and rejected association restored the AP with actual socket-task
acknowledgement. A subsequent `/network` readback returned HTTP 200 and its form.
See `evidence/2026-10-23-native-network-fallback/`. This is Root provisioning
proof; successful station joining, joined-network mDNS, link-loss fallback,
repeatable reconfiguration and the appliance browser Mask remain unverified.

Native retirement now also carries a private exact-Play scope derived from the
actual revoked pulse/timer owners. Root issuance can finish only that issued
native Play, preserves its monotonic counter and any later independent control
reservation, and refuses reissuance at old provider generations. CPU conformance
checks foreign and duplicate finish, old-generation refusal, and separately
sealed fixture Plans selecting newer generations; strict lint and the ESP32
release build pass. This is not a physical second Play or reconstructed ready
offers. Executor reuse and explicit fresh Root preparation remain open.

The combined sensing/setup check now passes with explicit workspace ordering.
A private HTTP network request acknowledged during sensing remains queued as
`awaiting-native-retirement` through all three real, signed observations and
ordinary Body Faces. Verified quiescence, ordinary Lull and signed retirement
release the workspace; only then does Root admit one join attempt and restore
the AP on association rejection in the same Boot. Final Root heap use was 15,444
bytes with a 105,592-byte peak. See `evidence/2026-10-23-queued-network-setup/`.
The failed startup and concurrent-construction captures remain separate evidence.
This profile serializes those constructions; it does not yet provide production
host-driven pause/resume, a second native Play or durable overnight recording.

The finite owner run also interprets each newly projected Body Face through the
ordinary timed seven-segment Mask and authored TM1637 packet expression. It
prepares the independently selected colon-only hardware profile, Fahrenheit
colon-fraction Look, three-second temperature/RH alternation and finite lifetime
before release. `physical-show-<sequence>.json` retains the exact Face content
identity/revision, elapsed Mask time, four segment patterns, separate
annunciators, six mapped registers and finite Source-encoded packets. Its
`physical_delivery: false` field is intentional: this owner-side interpretation
has no GPIO effects and is not an admitted display output Line or Host Call.
The existing native construction proof covers both physical Face replay sets
and this owner path, including stale observations, lifetime expiry and brightness
refusal. Native output admission, resident page delivery and physical verification
remain required before claiming the MCU display has resumed updating.

The optional TM1637 `host-call` feature now supplies exact selected output
possession around the existing wire engine. The authored packet expression can
return canonical `Tm1637Packets` through `PreparedDisplayPackets::encode_canonical`,
using reserved storage; the borrowed-packet API uses that same encoding. The
`display-output` proof combines its owner/negative tests with existing runtime
packet and native Face/Mask regressions. This is a CPU-verified output component,
not assigned firmware installation, admitted display work, a resident page loop
or evidence that the physical MCU display has resumed updating.

The retained TM1637 provider component now projects readiness through the existing
Base registry and binds either contiguous or borrowed assigned-plan records.
Refusal retains the exact provider in Root's slot; successful preflight transfers
it once without starting electrical work. The `display-output` proof includes
foreign Boot/resource claims, advertisement collision, invocation after assignment
storage is dropped and retirement of the original provider. Actual ESP32 provider
initialization, selected Back installation and resident delivery remain unfinished.

The display output component also includes `WriteBack`, a finite zero-result
Host Call sink under the production kernel. Its private phase changes only on
Step commit. The output proof executes two packet frames with the actual fixed
scheduler, selected owner and wire engine, covering electrical refusal,
cancellation, finite operation exhaustion and late completion rejection without
heap allocation. Routing, authority and pins are CPU fixtures. Native Mask Play
installation and continuous physical display delivery remain unfinished.

The native ESP32 composition now initializes and retains the real GPIO18/19
output provider and projects its Ready offer for the observed Host/Boot. This
starts no packet transmission and installs no Mask Play. The bare advertisement
keeps its 8 KiB limit, while its admission envelope has a separate 320-byte
reservation and the host bootstrap reader admits 12 KiB diagnostic frames.
The release link preserves the 28 KiB stack floor and both heap totals by moving
384 bytes between the two existing Root regions. See the size/link evidence in
`evidence/2026-10-24-native-display-root-size/`; actual output selection, resident
page delivery and physical display verification remain unfinished.

The host admission verifier uses the same 12 KiB carrier-frame ceiling as its
UART bridge; this is separate from Root's advertisement and envelope bounds.
The assignment-staging diagnostic now sends 32-byte bursts with 10 ms pauses,
without retries. A physical repeat passed malformed-input and duplicate-stage
refusals plus accepted staging on the same firmware after management reset.
The earlier unpaced duplicate diagnostic timed out after accepted staging; no
Body had started before that reset.

The first physical native preparation with output readiness correctly refused
`kernel-capacity` before Body start: the conservative kernel/control bound was
248 bytes above available heap. Root now validates the immutable display
contract at bootstrap and reconstructs its decoded metadata only during offer
or binding preparation, retaining the actual GPIO owner throughout. No kernel
admission guard, control headroom or heap total was reduced. The corrected
release builds; its native execution still requires the physical repeat.

The physical repeat of the metadata correction passed on firmware source
`055673a4d`: observed output readiness, Body admission, paced assignment staging,
signed native preparation/start/release, three real one-minute DHT observations,
ordinary Body Faces and timed Mask Show/packet interpretation, then verified
quiescence, Lull and provider retirement. Root peaked at 105,860 of 114,688 heap
bytes and ended at 10,380 bytes. See
`evidence/2026-10-24-native-display-readiness/`. Output remained unselected and
the Shows explicitly record no physical delivery. The device is quiesced after
the finite run; this does not claim a resident or overnight appliance.

`DisplayOutputSource` checks the authored `display-output.conduit` against the
existing protocol Types and the electrical sink's exact Fore. Packet encoding
is a closing Flow, so timed pages can share one finite invocation. The packet
result is routed to the write sink or the explicit `invalid_brightness` Fore;
an invalid brightness is not acknowledged as a display update. Source contains
no pin, Host, implementation, grant or hardware-profile binding facts.

`PreparedDisplayOutput` uses ordinary placement, planning, sealed Fore and
lowering over independently supplied current offers, Bases and authority. It
admits one-item Frame/refusal queues with explicit byte ceilings and refuses
missing output readiness/Base, missing authority or a foreign-Boot grant. The
planning tests retain actual initialized portable expression/selector backs;
pins and authority remain explicit CPU fixtures, with no I/O.
The `display-output` proof includes these checks plus existing packet, Mask and
native-owner regressions. This construction material does not install a Mask
Play, start a timer or transmit packets; those remain separate Root work.

`DisplayOutputSource::prepare_software` now initializes the explicitly selected
portable computations through the existing retained software owner. It binds
Source/checked/expanded identities, actual Host/Boot and projected offer
generation; ordinary exact plan selection transfers each object once. Unknown,
duplicate or electrical gears refuse portable preparation, and selected
objects disappear from fresh offers. Selector workspaces are reserved before
Play. The electrical provider and complete kernel/work admission remain
independent requirements. The display proof checks these ownership paths;
the existing ten appliance-kernel tests also pass after sharing initialization.
This does not change the flashed firmware or claim physical display delivery.

The optional runtime `display-output` feature prepares the checked output Plot
as one bounded ordinary kernel: four nodes, five Cords including Frame/refusal
Fore, twelve 1 KiB value slots and 256 event/remote-sign slots. Whole-Plan-aware
lowering must match exactly; activation entries, nonlocal Lines, unsupported
shape and bounds refuse. Numeric tables and all software selections are
preflighted before the first software transfer. The independent Root still
owns actual capability issuance, the electrical provider, complete memory/work
admission and Body/Play start; this constructor supplies none of those facts.

The display proof executes two successive register Frames through the checked
Source encoder, both selected selectors, original kernel Host Call requests,
and the production TM1637 capability owner/wire engine. Explicit CPU fixtures
supply Root authority and an acknowledged pin provider. Canonical requests
match the Source encoder; invalid brightness reaches the refusal Fore with no
pin effect, and cancellation before dispatch rejects a late completion with no
pin effect. Recurring execution measures zero allocations. Refused preparation
preserves all still-retained software. This is CPU execution evidence, not an
ESP32 installation, live timed Mask subscription or physical display update.

`DisplaySourceMaterial` now emits trusted firmware construction data rather
than requiring the complete expanded graph in Root. The existing flat-image
producer refused this output graph at its unchanged 32 KiB ceiling. The Make
material separates checked gear metadata from computation text, decodes one
gear at a time and initializes through the same portable implementation/contract
checks. Current metadata totals 16,589 bytes with a largest gear of 12,841 bytes.
The trusted Make entrance admits at most eight operations, requires independent
artifact/Source authentication and creates no native authority or Play.

A CPU proof reconstructs the actual software from this material, projects its
retained offers and selects/prepares the same output kernel. Host-only allocation
measurement reports an 88,273-byte preparation peak; that is not an ESP32 heap
bound or evidence that sampling and output preparation fit together. The native
build embeds and type-checks the generated constructor, and the supported
`root-esp32` release build passes the unchanged 28 KiB stack floor. Its preparation
entry is not called at bootstrap. Authenticated output selection, combined
memory/work admission, actual preparation on the MCU and physical output remain
unfinished; this build was not flashed.

The separate `display-ready-root` firmware profile initializes and retains the
trusted output software under a conservative 96 KiB preparation allowance plus
8 KiB control reserve. It omits the sampling software closure in this Boot and
starts no Body/Play or electrical transaction. Readiness and refusal appear in
Root's bounded status envelope; hardware pins stay independently retained.
This profile measures native initialization before combined sampler/output
admission and is not an appliance deployment.

Build it with `cargo xtask prove environment display-output esp32-readiness`.
Adding `--flash` explicitly flashes the attached known ESP32, saves its ELF and
source/SHA receipt and runs bounded read-only UART readiness capture. The proof
requires the expected MCU, output software ready, sampling software absent, no
installed Body/Play and valid Root heap counters. It records a refusal before
failing; it neither retries a mutation nor claims physical display delivery.
The ordinary `root-esp32` and `owner-run-esp32` paths keep their sampling profile.
