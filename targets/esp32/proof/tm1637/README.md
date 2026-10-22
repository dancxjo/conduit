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
