# Pico W USB / HTTP transport bring-up

This standalone native RP2040 proof forwards actual HTTP bytes between USB CDC
and the fixed ESP access-point peer `192.168.4.1:80`. It creates no Body,
membership, sensor reading, Source admission or Mask. It is a temporary reusable
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
Pico flash before separately authorized hardware flashing. GPIO 3/4/6/7 are
untouched. Existing CYW43 firmware assets and licenses are reused from the pinned
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
A future combined image must initialize the actual sensor and display, then use
existing shared Source/Body admission and leases; it must not invent Pico-only
observations or membership. GP6/7 are reserved for I2C1 and GP3/4 for TM1637.
