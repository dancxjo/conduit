# DHT11 electrical diagnostic

This finite Pico W appliance captures 32 transactions from GP2, three seconds
apart, and publishes raw five-octet frames or explicit timing failures over USB
CDC. It does not run the logger, Wi-Fi, a Conduit Host, or a second execution
kernel. GPIO ownership and pulse timing are specific to this diagnostic.
Successful capture does not imply a valid checksum or a usable observation.

Connect DHT11 DATA to GP2 (physical pin 4), ground to GND, and a module rated
for 3.3 V to 3V3(OUT). Use the module's labelled pin order. A bare sensor needs
an external DATA pull-up; a breakout may already contain one. Keep DATA and
its pull-up at 3.3 V. Disconnect USB before wiring. No other modules are used.

The repository entrances are:

```sh
cargo xtask prove dht11 check
cargo xtask prove dht11 build
cargo xtask prove dht11 build-source
cargo xtask prove dht11 build-pio
cargo xtask prove dht11 build-kernel
cargo xtask prove dht11 decode /absolute/path/to/capture.ndjson
```

`build` produces
`target/thumbv6m-none-eabi/release/conduit-pico-dht11-probe.uf2` within this
directory. Build and image inspection establish compilation only. Put the Pico
in BOOTSEL mode and copy this exact UF2 to its RPI-RP2 volume. This replaces the
previous firmware. The application enumerates as **Pico DHT11 Probe**, with
serial number `conduit-pico-dht11-probe`.

The diagnostic waits for a serial consumer to assert DTR, allows two seconds
for power settling, then captures 32 records. Record the resulting NDJSON
without adding terminal text. GPIO edges have finite timeout budgets. Each
transaction uses a 20 ms low start pulse and a bounded interrupt-masked capture;
this timing owner is unsuitable for claiming concurrent Wi-Fi operation.
USB receipt writes have a one-second deadline. Restart the appliance for a
second attended run; it does not conceal retries or run forever.

`build-source` adds an on-device check of the same checked Source expression
and generated quantity Form codecs. The build checks/expands the authored
decoder and embeds its canonical program; it does not translate the decoder
into a separate native implementation. Canonical capture composition uses
prepared, finite buffers. After preparation the allocator is sealed, five
target adapter checks exercise positive/negative readings and short/checksum/
invalid-payload refusals, and the diagnostic captures its 32 physical frames.
The USB records retain raw octets and first-class Temperature/Ratio coordinates.
They use the `conduit.dht11-source-probe/v1` schema, including a preparation
record with the exact Source digest and measured heap use.

The diagnostic preparation heap is 96 KiB. A physical RP2040 run measured
79,004 bytes at preparation peak and 42,264 bytes retained. This includes the
borrowed-composition adapter's preparation, allocator alignment and bookkeeping;
it is not a stack measurement or complete logger admission. The native host
conformance separately bounds its own 64-bit preparation footprint. These
different measurements must not be substituted for one another. The Pico Host
specification now requests a 96 KiB preparation ceiling; full kernel/radio/history
composition still needs its own exact admission and footprint proof.

All diagnostic profiles enumerate with manufacturer `Conduit`, matching the existing
headless-development udev rule. After the finite diagnostic retires, they reuse
the production firmware's exact-build BOOTSEL challenge/ACK handler and the
existing bounded stream framing. The ordinary `cargo xtask make pico bootsel
--link-port <device>` command can request that reboot. The initial firmware
without this handler still needs a manual BOOTSEL reconnect. A previous
BOOTSEL FAT mount can remain after USB disappears; retire it through the
installed headless helper before mounting the newly enumerated volume.

On-device Source evaluation is a distinct proof class from the host transcript
decoder below. The preceding electrical/Source profiles do not execute a production Plan/Play
or supply autonomous logger behavior. Temperature Unit selection for the seven-segment
Mask remains an independent Look; the observation retains its semantic quantity.

`build-pio` additionally selects the generic PIO/DMA electrical realization.
The DHT11's geometry comes from the checked `dht11-capture-request` Source
entry, through the same exact request codec used by the portable pulse Fore.
The request is prepared before the decoder so their temporary preparation
storage can be reused. A physical run measured the same 79,004-byte peak and
42,264-byte retained heap as the original decoder profile. The diagnostic
reports the separate capture Source digest and `capture_owner: pio-dma`.
PIO1/SM0 and DMA2 own timing and capture; IRQs remain enabled for the frame.
The legacy interrupt-masked capture remains fenced to the other diagnostic
profiles. Neither path installs a production logger capability.

`build-kernel` selects a finite autonomous four-read bench, three seconds
apart, using the authored environmental sampling Plot and the existing production
kernel. The ordinary workload Source in
`bodies/environment/profiles/sampling.conduit` defaults to 960 samples one minute
apart (sixteen hours). Its full overnight run and history retention are not yet
physical proof. The shorter bench admits exactly four pulse operations.

Make checks the 15-node/14-Cord graph and round-trips an inert rich Plan template.
At runtime the Root selects a fresh Boot, reseals the template through ordinary
Core, binds actual PIO/DMA ownership and prepares the kernel. Fixture-prefixed
identities fence this bench from installed Body membership and Host availability.

Sampling starts after a two-second settling interval without waiting for USB or
DTR. The native wait handler retains one pending deadline while the kernel and
sensor work advance; it does not block the kernel behind the next interval.
After the finite Play drains, an optional USB reader may inspect the retained
latest result and occurrence counts. The bench retains only the latest result
per outcome, not an overnight history. Face construction is a separate preparation
phase after the kernel drains. Its diagnostic freshness budget is twice the
selected sampling period; a late reader gets the stale `----` Show.

Physical proof on 2026-10-22 completed four DHT11 reads at 3.001–3.002-second
intervals with allocation sealed during Play. A second run completed all four
before a USB reader attached, with no timeout or provider failure. The late
attachment rendered both stale values as `----`. The earlier fresh run rendered
`66.2F` and ` 55H` through the existing Face and Fahrenheit Mask. Observations and
Face retain semantic Temperature/Ratio quantities; the Look changes expression.
No physical display is wired.

The bench uses a 196 KiB arena (200,704 bytes); its measured peak was 195,564
bytes, with 156,384 bytes retained during sealed Play. Its ELF reserves 213,752
RAM bytes, leaving a 56,584-byte stack region. The async poll frame is 19,376
bytes by disassembly, not a whole-call-stack high-water measurement. Large
construction moves and kernel calls are outlined to keep preparation temporaries
out of unrelated phases. These measurements do not admit the production Host's
separate 96 KiB ceiling or the future radio/history composition.

The semantic observation Cord admits 4,096 bytes; other Cords remain at 1,024.
All kernel stores and Back storage are reserved before Play. The reusable time
Back now lives in the portable time package; its counted mode retains one wait
rather than one stored value per sample. The typed tick-to-trigger Back validates
the eight-byte Tick and emits Empty. The general expression engine does not
pretend that arbitrary native Tick bytes are its checked primitive input format.

The recovery handler returns to BOOTSEL after panic, allocation refusal or
HardFault. Reflash the identical image to inspect its retained fault receipt;
recovery waits for lifecycle control rather than retrying the read. Automatic
fault recovery and exact-ELF software BOOTSEL reboot were exercised during bring-up.
Prior failed images are diagnostic evidence, not accepted cadence proof.

The earlier 13-node raw-decoder bench completed one kernel read and lifted
quantities after Play (187,544-byte heap peak). The following 14-node semantic
bench completed quantity construction inside Play and post-Play Face/Mask output
(191,372-byte peak). Their preserved receipts describe those exact images.

`decode` evaluates the actual `plots/device-protocols/dht11.conduit` wire decoder
on the host, then applies its checked Forms to construct the observation's
first-class `Temperature` and `Ratio` values. The temperature retains its affine
point role and °C Unit; humidity retains the % Unit. The Forms independently
select two-byte integers in tenths. Raw bytes, capture timestamps and unavailable
outcomes are retained. The representation does not increase sensor accuracy.
At most 32 records and
64 KiB of input are accepted. Host expression evaluation does not establish
MCU kernel execution, all-target portability, or physical provenance of an
arbitrary supplied file. Retain the actual USB capture and the exact UF2 digest
separately when recording physical proof.

These Forms generate native Rust codecs. The ECMAScript Form generator reports
`unsupported_storage` for quantity Forms; it does not treat them as enum tags.

The checksum and extended measurement ranges follow the
[Aosong DHT11 V1.3 manual](https://akizukidenshi.com/goodsaffix/DHT11_20180119.pdf).
Fractional and negative temperature interpretation follows the explicit DHT11
encoding used in the
[Adafruit driver](https://github.com/adafruit/DHT-sensor-library/blob/master/DHT.cpp).
Reserved fractional bits and readings outside the extended DHT11 range refuse.
This is the DHT11 profile; it must not be used to interpret DHT22 frames.

The production work still needs admitted MCU capture and clock Backs, execution
autonomous operation without a USB consumer,
bounded overnight history, an AP/browser encounter, and the remaining MCU
realizations. This diagnostic is physical bring-up evidence for that work.


## Attended display electrical fixture

`cargo xtask prove dht11 build-display` builds a separate feature profile for
CLK GP3 and DIO GP4, without an enable GPIO. It writes reviewed test vectors
`8888` and `1234` at brightness 2, stopping on the first NACK. USB receipts report
idle digital levels and exact failed packet/byte locations. These vectors are
not sensor values or the installed Face/Mask path. The physical module's address
and segment mapping still require confirmation. Lifecycle control remains available
after the finite test. Panic/HardFault receipts survive a ROM BOOTSEL reset when
the identical image is reflashed. This is not production Host installation.


## Environmental AP and HTTP bench

`cargo xtask prove dht11 build-http` builds the DHT11 GP2 production-kernel
bench with a read-only AP service. Join the open **Conduit-Climate** network and
open **http://192.168.4.1/**. `/reading` returns JSON. DHCP admits four client
leases; three network socket slots provide DHCP plus two HTTP connections.
Requests have a 512-byte ceiling and a five-second transaction deadline;
responses have a 2048-byte ceiling. Other paths receive 404.

The authored finite Tick/Trigger/Observe composition samples every 15 seconds,
4096 times (about 17 hours). Source checks sensor frames and the observation
Back emits first-class Temperature and Ratio quantities. The HTTP Look converts
temperature to Fahrenheit downstream. Latest committed readings are published;
no overnight history or persistent browser store is claimed. Sensor failure
preserves the last valid values with an unavailable status. Values older than
30 seconds are marked non-current. The page refreshes every five seconds.
Sampling and AP startup do not wait for USB. Existing lifecycle control remains
available during the timer wait. No external display pins or shifter are used.

The existing CYW43 radio setup, firmware assets, DHCP parser and lease pool are
reused. Radio PIO0/SM0/DMA0/1 and sensor PIO1/SM0/DMA2 share one DMA IRQ binding.
Network state and its future are prepared after the rich Plan preparation peak,
inside the finite arena, before allocation seals. No network allocation is
permitted during Play. This is still a fenced physical bench using fixture
identities, not an installed production Host/Body workload or browser Line.
The production construction profiles' smaller limits are not silently enlarged.

`cargo xtask prove environment http` checks Fahrenheit conversion, signed values,
freshness, failed observations retaining timestamps, JSON and HTTP content length.
Physical AP/HTTP evidence and exact firmware hashes are retained by the invoking
task separately from portable tests. Do not infer a full 17-hour endurance proof
from a short physical run.
