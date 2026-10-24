# ESP32 TM1637 electrical diagnostic

Attended fixture for the HW-463 ESP-WROOM-32 board. This invokes the existing
finite TM1637 wire transport directly with explicit test vectors. It is not a
Body, an admitted production Host Call, a Mask realization or a sensor reading.

Connections: display CLK → GPIO18/D18, DIO → GPIO19/D19, VCC → 3.3 V and
GND → GND. The observed setup uses the carrier S pins for signals, with the
adjacent V/G rails and its voltage jumper selected to 3.3 V. No level shifter.
The GPIOs only sink low or release; internal pull-ups are enabled. Both input
buffers remain enabled so the provider can observe the real ACK bit.

With the repository's pinned ESP32-WROOM SDK installed and activated:

```sh
cargo xtask prove environment display-esp32
```

This builds only. Flashing requires an explicitly selected attached board and
the pinned ESP32 flasher. The diagnostic cycles through all segments, `1234`,
and `67.6F` every three seconds. The number is a fixture, not a measurement.
Each transaction reports its ACK result on the development board's USB UART
at 115200 baud. The diagnostic consumes only GPIO18 and GPIO19.

On 22 October 2026 the actual ESP32 revision 3.0/4 MB board acknowledged all
three vectors with a 100 microsecond half-period. The user visually confirmed
`1234` and `67.6F`, including the point after 7. Both GPIOs were observed low
when driven and high when released. Earlier 10 microsecond runs returned NACKs;
the retained profile uses the observed working timing without generalizing that
result to other modules or the earlier RP2040 provider. The other decimal points
and any independent/shared colon mapping still require separate verification.
Production integration must admit the actual electrical work and resource
ownership before calling this provider from a planned display operation.

## Capture integration fixture

`cargo xtask prove environment capture-esp32` builds the same attended fixture
with the new ESP32 pulse provider on GPIO27/D27. It samples once per complete
three-pattern cycle using the explicit DHT11 capture-request test vector and
prints complete octets on UART. No native checksum or sensor decoder is added.
A rejecting global allocator admits no heap; unexpected capture-time allocation
would fail rather than silently create storage. This is physical capture evidence,
not execution of the Source decoder or a live temperature Show.

On the connected board the initial capture was `[106,0,40,4,151]`; subsequent
captures were `[53,0,20,2,75]`. The transport reported complete five-octet captures,
and all display transactions remained acknowledged. Preserve the initial frame
as evidence too; completeness is not sensor validity. The real Source decoder
must validate checksum and range and publish explicit failure when appropriate.
Radio coexistence, full appliance installation, AP/HTTP and a sensor-driven
Fahrenheit Show remain integration work.

## Live Source and AP integration bench

`cargo xtask prove environment live-esp32` builds the Source capture-request,
Source decoder, quantity Face, Fahrenheit Mask and Source packet encoder with
the actual pins above and DHT11 DATA on GPIO27. Invalid frames render `----`.

`cargo xtask prove environment ap-esp32` also builds the ESP32 radio, finite
DHCP service and two bounded HTTP sockets. The attended network is the open
`Conduit-Climate` AP, at `http://192.168.4.1/`, with JSON at `/reading`.
The bench reuses the existing portable DHCP grammar and HTTP service from the
RP2040 proof, without linking its HAL. Sensor and HTTP Looks share one Face.
The current display hardware profile only admits the verified decimal point
after digit two; other dots and colon remain unverified.

These builds remain explicitly prebirth integration benches, with no Body,
Wake, Plan or active Play identities. AP-to-station selection/fallback and
recorder publication are not installed. Native Host Call admission, radio
coexistence bounds and lifecycle installation still need their own proof.

Physical AP integration on 22 October 2026: DHCP assigned the host an address;
Ethernet stayed connected; both `/` and `/reading` returned HTTP 200. Successive
JSON samples advanced sequence and Face revision, with 68.36°F and 52% RH.
The same Face produced the acknowledged `68.4F` segment pattern. The finite
capture loop is resident in RAM and observes release-high before clock
bookkeeping; the earlier flash-resident loop consistently produced shifted
checksum failures in this larger image. Raw failures remain retained evidence.
The verified AP build and its cross-target Clippy check passed. This short test
does not establish overnight endurance or native lifecycle admission.

The AP bench also reports a boot observation after radio entropy initializes:
a stable eFuse-derived hardware label and a fresh 128-bit TRNG boot nonce.
It refuses observation if the real entropy source is unavailable. This diagnostic
record grants no Host offer or Body installation authority, and leaves the image
attestation field empty; the host retains the flashed artifact digest separately.
Two physical boots on 22 October retained the hardware label and produced distinct
boot nonces. Both resumed valid sensor readings and acknowledged display writes;
HTTP after the second boot returned fresh 68.36°F and 53% RH. Startup waited for
actual DHCP and HTTP socket readiness. Service stop/restore remains unproved.

The AP bench retains distinct AP and station network stacks with four socket
slots each and two statically allocated interface runner tasks. The station
starts without an IPv4 configuration; DHCP is enabled only by the switch
operation. The composition root retains the controller, both stacks and the
actual AP configuration/address. This does not establish a station attachment
or admit a network Host Call. Physical startup with both runners on 22 October
reported 66,896 free heap bytes after radio initialization and resumed valid
68.36°F/53% RH readings with acknowledged display writes. Station association,
link-loss recovery and sustained coexistence remain unproved.


## Native Root assignment staging

With the native-root image running on a fresh Boot and UART0 connected through
USB, run from the repository root:

```sh
cargo xtask prove environment assignment-ingress-esp32
```

This captures actual initialized offers and Root policy, makes an ordinary
appliance Plan on the planning host, and sends its 8 KiB-bounded Source-reference
transfer to Root. Root checks the current Host/Boot, canonical assignment,
independently provisioned graph/configuration pins, roles and cadence before
retaining ordinary runtime tables. Wrong Source, stale Boot, corrupted canonical
digest and duplicate staging are exercised. Captures and receipts remain under
`target/environment-software-ready/<capture>/`.

Root retains one staged assignment until reset; repeating the proof in that Boot
refuses as already staged. The request has a five-second packet deadline and
both ready and result replies have two-second deadlines. It reuses the existing
8 KiB UART buffer and borrows Source computation records from firmware.

`src/appliance_layout.rs` is trusted Make-generated provisioning material,
separate from the received packet. Its full Source digest guard requires renewed
provisioning when checked meaning changes. Native call tails still need binding
to actual retained owners and local opaque authority. Staged tables are separate
from Body/Play installation and complete runtime memory admission; this path
starts no climate sampling, display output or recorder.

## Current physical display profile and live restoration

The later all-on and 48-register-bit scan superseded the initial visual dot
interpretation above: only the centre colon is connected, at address 1 bit 7.
The profile admits no bottom decimal points. The Fahrenheit Look preserves
tenths by explicitly choosing that colon as its fractional marker.

On 24 October the live AP bench was restored after native readiness experiments.
It alternates Fahrenheit and RH every three seconds and samples every fifteen
seconds. Thirteen captured writes were acknowledged, with three advancing valid
samples and an HTTP reading from the same Face. See
[the retained evidence](../../../../bodies/environment/evidence/2026-10-24-live-restoration/README.md).
This is useful bench behavior, not the installed native recorder appliance.
