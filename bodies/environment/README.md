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

The current 4096-slot Host request is still a construction request and must be
reconciled with the concrete chunk realization before live recorder admission.
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
content. A validated reader, cross-chunk recovery, actual publication completion,
finite generation admission and the producer Line remain integration work.

The ESP32 display now has an attended physical diagnostic under
`targets/esp32/proof/tm1637`. The selected GPIO18/19 wiring was exercised at a
100 microsecond half-period with open-drain release and internal pull-ups. All
segments, `1234` and the `67.6F` fixture were acknowledged; the user confirmed
both digits and the point after 7. This establishes that physical display path,
not an installed seven-segment Mask/Host Call or a real temperature display.
