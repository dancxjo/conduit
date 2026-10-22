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
The physical TM1637 wiring is deferred at the user's request.

Proposed TM1637 attachment: CLK → GP3 (physical 5), DIO → GP4 (physical 6),
both through an open-drain-compatible bidirectional level shifter. The user
identified the display supply as 5 V: VCC → USB VBUS (physical 40),
GND → shared ground (physical 8). The shifter's low supply uses 3V3 (physical 36)
and its high supply uses USB VBUS. No GPIO is assigned for enable; GP5 remains
free while the replacement shifter awaits identification. Disconnect USB before
wiring. Never connect 5 V display pull-ups directly to Pico GPIO. These are
proposed physical Host facts, not an installed driver offer or confirmation
that the display is wired. DHT11 DATA remains on GP2.

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
