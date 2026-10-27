# Pico W USB / HTTP transport bring-up

This standalone native RP2040 proof forwards actual HTTP bytes between USB CDC
and the fixed ESP access-point peer `192.168.4.1:80`. The transport creates no Body, membership or Source admission. Its independent
hardware diagnostic uses the shared environmental Face/Look. It is a reusable
Line adapter for reaching the ESP without changing the host's Internet interface.

The USB role is `conduit.pico-usb-http-relay@1`; the serial descriptor is
`conduit-pico-usb-http-relay`. The host refuses a different firmware role before
sending credentials. Credentials are supplied over USB, retained only in RAM,
and never written to flash. Owned mutable command/scratch buffers are wiped;
Python, USB drivers and vendor libraries may make other RAM copies.

Requests are limited to 8 KiB including HTTP headers. Responses are streamed in
512-byte USB frames and limited to 64 KiB; the proxy exposes a response only
after exact completion. Association, DHCP, TCP and USB operations have bounded
timeouts. Only a DHCP lease in `192.168.4.2..254/24`, with no default router or gateway `192.168.4.1`,
authorizes a TCP request. A failed HTTP operation is never automatically replayed;
a later distinct request may reconnect and rejoin.

## Build and proof

From the repository root (use the existing pinned lockfile):

```sh
CARGO_TARGET_DIR=target/pico-usb-http-relay cargo +stable test --locked --offline --manifest-path targets/rp2040/proof/usb-http-relay/Cargo.toml --lib --test protocol
CARGO_TARGET_DIR=target/pico-usb-http-relay cargo +stable clippy --locked --offline --manifest-path targets/rp2040/proof/usb-http-relay/Cargo.toml --lib --tests -- -D warnings
CARGO_TARGET_DIR=target/pico-usb-http-relay cargo +stable clippy --locked --offline --manifest-path targets/rp2040/proof/usb-http-relay/Cargo.toml --target thumbv6m-none-eabi --release --features firmware --bin conduit-pico-usb-http-relay -- -D warnings
CARGO_TARGET_DIR=target/pico-usb-http-relay cargo +stable build --locked --offline --manifest-path targets/rp2040/proof/usb-http-relay/Cargo.toml --target thumbv6m-none-eabi --release --features firmware --bin conduit-pico-usb-http-relay
python3 -m unittest discover -s targets/rp2040/proof/usb-http-relay/tests -p 'test_*.py'
elf2uf2-rs target/pico-usb-http-relay/thumbv6m-none-eabi/release/conduit-pico-usb-http-relay relay.uf2
```

The final command only converts a file; it does not deploy. Preserve the actual
Pico flash before separately authorized hardware flashing. The combined diagnostic now owns GPIO3/4 for the TM1637 and I2C1 GP7/6
for SCL/SDA. Existing CYW43 firmware assets and licenses are reused from the pinned
Pico firmware, as is its actual radio initializer; no Signal/Body build is run.

After the correct image is installed and its actual radio is verified, run with
Python `pyserial` available:

```sh
python3 targets/rp2040/proof/usb-http-relay/relay.py --port /dev/ttyACM0 --ssid 'ACTUAL ESP AP SSID'
# Add --ask-password for a protected AP. Do not put passwords in command arguments.
```

The server binds only `127.0.0.1:8766`. It does not install routes, change Wi-Fi,
or expose a general network relay. Transport Host/Connection headers are set for
the fixed peer; canonical HTTP body and response bytes are unchanged. Proofs use
fake transport endpoints; native compilation is not physical radio/USB proof.

## Integration boundary

The intended product remains the common environmental Source, Face, Mask and
native Body. Keep this USB/network framing adapter separate from measurement
semantics. Bind actual initialized hardware providers to that shared appliance:
DHT temperature/RH, BME temperature/RH/pressure, BMP temperature/pressure.
Unsupported humidity stays unavailable. Pressure is a real Quantity in Pa/hPa;
altitude is a separate computation requiring reference pressure. The Bosch driver
at `mechanisms/io/bosch-env` supplies hardware readings, not admission authority.
The diagnostic initializes actual sensor/display owners. Product integration
still requires existing shared Source/Body admission and leases; it must not
invent Pico-only observations or membership. The combined diagnostic uses GP6/7 for I2C1 and GP3/4 for TM1637.

## Developer reboot / BOOTSEL

The relay preserves the prior native firmware's explicit exact-build
query/challenge/request/acknowledgement recovery sequence, carried in its own
framed USB protocol (kinds 4/5 and 132/133). The 64-character SHA256 identity is
computed from the firmware sources, lockfile, linker layout and reused radio
assets. A wrong build, malformed payload or stale sequence cannot reboot it.
The device acknowledges the exact build, wipes its command buffers, waits
100 ms, then calls the RP2040 ROM USB boot entry. No HTTP route invokes this.
No Wi-Fi association is required:

```sh
/usr/bin/python3 targets/rp2040/proof/usb-http-relay/relay.py --port /dev/ttyACM0 --bootsel-build EXACT_64_HEX_BUILD_ID
```

Use the identity recorded with the exact ELF/UF2 build, rather than automatically
accepting any device challenge. The firmware does not implement implicit
1200-baud reset. Older relay images without this control need manual BOOTSEL.


## Combined environmental hardware diagnostic

The image also initializes the actual BME280/BMP280 at I2C address 0x76 on
GP6 SDA / GP7 SCL, with 100 ms whole-operation deadlines. It samples every
30 seconds independently of USB or ESP association. Failures clear the current
reading and require reinitialization; BMP280 humidity is explicitly unsupported.
The common EnvironmentalReading adapter preserves exact centi-Celsius,
centi-Pascal and Q10-percent values. The shared environmental Face contains
pressure; the existing shared Mask alternates whole-degree Fahrenheit and RH
on GP3 CLK / GP4 DIO every 2 seconds. Pressure has no invented display page.
Failed or unsupported values use the existing unavailable glyphs.

This is a hardware diagnostic composition of shared Face/Look, not an installed
Body/Plan: its Face has no Body, Wake, Source, Plan or active Play identity.
Ordinary Face preparation uses a fixed 48 KiB heap. Sampling and display do not
wait for a host. The USB transport and exact-build BOOTSEL remain separate.
Control kind6 (`SENSOR_STATUS`, empty payload) returns JSON before Wi-Fi JOIN:

```python
status = relay.Relay(serial_port).exchange(relay.SENSOR_STATUS)
```

It reports verified `chip_id` (96 BME / 88 BMP), configured `address` (118),
measurement attempts/successful `samples`, display `frames` attempted and `acks`
completed, error status and heap peak. A completed ACK count means the actual
wire transmitter accepted all 9 bytes; it is not human visual confirmation.
Successful readings include `temperature_centi_celsius`,
`pressure_centi_pascal`, and `humidity_percent_q10` (`null` for BMP unsupported).
The broader INFORMATION response bound is now512 bytes; sensor JSON uses a
fixed384-byte formatter. No provisioning or signed-history format changed.

When uninitialized, the diagnostic probes 0x76 then 0x77 once each, with a
100 ms deadline per initialization. It moves the same exclusive I2C bus between
attempts. Only a supported chip ID plus validated calibration/configuration
makes a sensor ready. `address` reports the selected attempt, not proof that
anything acknowledged it. Kind7 (`SENSOR_DIAGNOSTIC`, empty payload) retains the
actual driver initialization errors for both addresses, measurement errors,
TM1637 failure (including Nack packet/byte), and released digital line levels
before transmission and after cleanup. These levels are not voltage readings.
Both status controls remain available without an ESP Wi-Fi association.
