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
a dots-off `1111`, bit 7 of each of the six controller registers separately,
and `67.6F`, each for three seconds. Registers 0–3 test the expected four
decimal points; registers 4–5 also probe any additional wired annunciators.
Only visual observation can establish whether a bit lights a point, colon or
shared elements. The numbers are fixtures, not measurements.
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

The separate autonomous sampler/Mask profile is built with
`cargo +stable xtask prove environment local-appliance-image <owner-public-key-file>`.
Supply the authenticated birth Sign through
`CONDUIT_ENVIRONMENT_BODY_BIRTH_SIGN`; retain the existing Body checkpoint and
archive partitions when updating an installed appliance. This command builds
and checks packaging; it does not flash. The combined profile uses the pinned
ESP toolchain, size optimization, fat LTO and an inlining threshold of ten for
both ordinary and hinted calls. The physical image with the shared `H 44` /
`H100` humidity Look occupies 3,318,048 of 3,371,008 app bytes with this recipe.
Its Root heap, SDK heap, stack floor and five history epochs are unchanged.


The replacement ESP32 revision 3.1/4 MB board was flashed with the expanded
annunciator probe. Serial ACKs establish accepted writes, not visible dot/colon
mapping; that human observation is recorded separately.
