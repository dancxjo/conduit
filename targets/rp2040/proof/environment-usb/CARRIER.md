# Pico W selected-peer HTTP carrier

`carrier.py` exposes a loopback HTTP endpoint through the shared firmware role
`conduit.environment-wifi-carrier@1`. It rejects the older USB relay role. The
carrier remains `body_member:false`: forwarding traffic and observing DNS do
not establish Body membership or authenticate the selected Root.

Install the normal Python `pyserial` dependency in the host's existing tooling
environment. Prepare an owned private `conduit.private/known-wifi@1` file using
`bodies/environment/host/tools/prepare-known-wifi.py`. The carrier accepts one to
four profiles, selects the exact `--ssid`, and currently requires
`security: "wpa2-personal"`. The file must be an owned regular 0600 file in an
owned private directory; symlinks, duplicate fields/SSIDs and oversized inputs
are refused. No password is accepted on the command line or logged.

After independently identifying the actual Root's private on-link IPv4 address,
run one process against the explicitly identified Pico CDC device:

```sh
python3 targets/rp2040/proof/environment-usb/carrier.py \
  --port /dev/ttyACM1 \
  --network-file "/run/user/$(id -u)/conduit-known-wifi.json" \
  --ssid Troys --peer ACTUAL_ROOT_IPV4 --http-port 8767
```

`ACTUAL_ROOT_IPV4` must be replaced explicitly; it is not the carrier's own
address. The firmware checks its actual DHCP subnet and refuses self, off-link,
network and broadcast endpoints. The target TCP port is fixed at 80. No scanning,
SSID discovery, NetworkManager changes, host routing changes or Internet changes
are performed. The listener binds only `127.0.0.1`.

Each USB session exclusively owns its CDC stream, holds DTR low for 650 ms,
discards stale input, starts a fresh sequence, validates STATUS, explicitly JOINs
the chosen protected network using canonical CNJ1 bytes, and SELECT_PEERs the
explicit address. A previously up link does not substitute for JOIN because
STATUS omits its SSID. Peer acknowledgement must match the new attachment's IP,
generation and firmware identity.

HTTP forwarding preserves complete peer response bytes, waits for the matching
DATA/DONE extent, enforces the existing 8 KiB request / 64 KiB response limits,
and suppresses HTTP request/path logging. An interrupted or refused request
returns a transport error and is **never replayed**. Only a later distinct HTTP
request may reopen USB, reread the private profile, join and select again. All
retained credential bytearrays are wiped whenever that session closes. Stop the
process before using another tool on the same CDC device.

The Python `observe_mdns()` API supports optional firmware command 10 only after
this session has selected the peer. It correlates the nonce, exact MCU hostname,
source/address, port and attachment generation. This is an observation, not
Root authentication; HTTP forwarding never invokes it automatically.

CPU verification (injected USB streams and local fixture HTTP only):

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s targets/rp2040/proof/environment-usb/tests -p test_carrier.py -v
```

These tests do not prove physical USB, association, on-link routing, DNS, Root
membership or autonomous measurement. Those require the separately recorded
hardware and signed Root evidence.

The same carrier supports distinct checked Host configurations:

- `pico-w-environment-usb.host.conduit`: Bosch environmental sensor and HT16K33 on I²C (Green).
- `pico-w-dht11-tm1637.host.conduit`: authored DHT11 capture/decoder and GPIO TM1637 (Yellow). Build with `--features wifi,dht11-tm1637`; the checked profile selects GP3 data, GP7 CLK, GP6 DIO,50µs electrical half-period and weak pull-ups. Pressure is unavailable on this sensor and is omitted.

Both perform local continuous diagnostics independently of USB and report `body_member:false`. Neither diagnostic is an installed same-Body sampling/Mask Play, and current Wi-Fi credentials are session RAM rather than restart-persistent configuration. The host explicitly establishes the carrier session after a power cycle.

When several devices are attached, select the actual physical USB path and verify its build/configuration. These firmware variants currently share a USB product/serial descriptor, so the shared `/dev/serial/by-id` alias can move between devices. Never open the same CDC stream from two clients. Display ACKs are electrical observations; working pixels require physical visual confirmation.
