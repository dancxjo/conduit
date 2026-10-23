# Environmental observation Body

The construction specifications are Conduitese, consumed by the existing Host
and Body checkers:

- `profiles/environment.body.conduit` admits the persistent sensor part alone.
- `profiles/environment-browser.body.conduit` retains the same Body and part
  identities and adds a self-joining browser recorder. The recorder is optional;
  its invitation name requests a binding and does not mint authority.
- The sensor Host selects the registered Pico USB implementation and declares
  finite GPIO, clock, local retention and access-point resource needs.
- The recorder Host selects the registered browser Face, WebSocket and IndexedDB
  implementations, with a finite durable-storage budget.

Check these exact files without building firmware, deploying Spores or starting
Play:

```sh
cargo xtask prove environment specifications
```

This focused entrance uses the real RP2040/browser make packages and shared
Host/Body checkers. It checks both compositions and canonical Source round trips.
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
acknowledgement, recorder membership/Line, cross-chunk recovery and physical
endurance are still required for a deployed recorder appliance.
