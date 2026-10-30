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
request may reopen USB, reread the private profile, join and select again. The Python client
wipes its retained credential bytearrays whenever that USB session closes. Firmware
retains the selected credential in volatile RAM across USB disconnection for
automatic carrier reconnect; replacement wipes it, and power loss discards it. Stop the
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

- `pico-w-bme280-tm1637.host.conduit`: BME280 on I²C1 (GP6 SDA/GP7 SCL) and TM1637 on separate GPIO (GP3 CLK/GP4 DIO), White. Build with `--features wifi,bosch-tm1637`; bindings and electrical timing come from the checked Host configuration.
- `pico-w-environment-usb.host.conduit`: Bosch environmental sensor and HT16K33 on I²C (Green).
- `pico-w-dht11-tm1637.host.conduit`: authored DHT11 capture/decoder and GPIO TM1637 (Yellow). Build with `--features wifi,dht11-tm1637`; the checked profile selects GP3 data, GP7 CLK, GP6 DIO,50µs electrical half-period and weak pull-ups. Pressure is unavailable on this sensor and is omitted.

All three perform local continuous diagnostics independently of USB and report `body_member:false`. Neither diagnostic is an installed same-Body sampling/Mask Play, and current Wi-Fi credentials are session RAM rather than restart-persistent configuration. The host explicitly establishes the carrier session after a power cycle.

When several devices are attached, select the actual physical USB path and verify
its build/configuration. New images use the flash device's eight-byte physical
unique ID as a fixed 16-character hexadecimal USB serial. That serial survives
firmware and sensor/display profile changes, so each board gets a distinct
`/dev/serial/by-id` alias. It is physical presentation metadata, not a Conduit
Host/Boot identity, membership proof or authority. If the read fails or returns
an unprogrammed placeholder, USB omits the serial rather than inventing a shared
fallback; local sensing still starts. Use `/dev/serial/by-path` in that case.
Older images still report `conduit-environment-usb` and can collide until updated.
Never open the same CDC stream from two clients. Display ACKs are electrical
observations; working pixels require physical visual confirmation.

All Wi-Fi profiles expose the same bounded read-only `GET /environment` route on
TCP port 8080 after network configuration. Its response carries the firmware and
Host-configuration identities, real counters, sample-Boot milliseconds, quantities
and explicit `body_member:false`. It creates no Body membership, signed history,
Plan or Play. There are two fixed diagnostic TCP sessions so an idle browser preconnection
can coexist with its navigation. Each closing handshake is bounded to 500 ms;
further concurrent connections can refuse. These sockets grant no Line authority,
and the checked `line_sessions: 1` remains unchanged. Their four RX/TX buffers
occupy 4096 bytes. Including each session's larger response phase (1536-byte
maximum status plus 192-byte header) gives a 7552-byte peak buffer budget,
checked against the existing 8192-byte buffered limit. Request accumulation
ends before response construction. Four
network socket slots cover DHCP, both diagnostic sessions and the serialized
carrier HTTP/mDNS operation. Full static memory and stack bounds remain enforced. Incomplete
requests expire after three seconds; headers are limited to 512 bytes and methods
other than GET refuse. Credentials and admission operations are not exposed there.

Physical evidence on White established successful joins to Troys and `Hot Togo `
(the latter includes a trailing space), direct Wi-Fi status reads on Hot Togo,
and continued sampling/display acknowledgment progression with no USB client.
The shared endpoint and sample-Boot timestamps are compiled for Green and Yellow
as well; each profile reports only quantities its actual sensor provides.
A protected RTL8188CUS bench AP rejected association and remains unqualified.
Network credentials are still volatile; these observations do not establish
automatic reconnect, power-cycle recovery or native Body membership.

The shared carrier retains one fixed-capacity (167-byte encoded maximum) selected
join request in volatile RAM and retries that same WPA2 network after link/address
loss. Failed attempts wait 5, 10, 20, 40 and then 60 seconds between attempts;
there is no scan, network ranking, NAT traversal or authority change. A station
still associated without an address gets a 60-second DHCP grace period, and late
DHCP completion cancels the pending retry and clears the carrier error. Each retry
invalidates the previous attachment before joining; a superseding explicit request
wipes the old retained credentials. The existing per-attempt leave/join/DHCP limits
remain 5/25/15 seconds. Local sensor/display tasks remain independent. Timing tests
and firmware compilation establish this implementation; physical outage/rejoin
acceptance and restart-persistent credentials remain unproved.

USB join/peer replies carry correlated, exhaustion-checked request tokens. A queued
join starts only when its remaining caller deadline can cover all 45 seconds of
existing join phases; otherwise it refuses before changing credentials. An expired
peer selection also refuses before changing the attachment. A timed-out caller
cannot accept a later operation's reply. Reconnect does not cancel an in-flight
CYW43 operation; an explicit request arriving during one can therefore refuse and
must be submitted again. Successful address recovery clears only network-phase
errors, preserving attachment-capacity failures.
