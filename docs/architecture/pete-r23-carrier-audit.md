# Pete R23 carrier audit

**Status:** diagnostic source note for the installed R23 carrier, not a physical acceptance record

**Owning investigation:** [issue #1687](https://github.com/dancxjo/conduit/issues/1687)

**Source:** user-supplied R23 `kicad_pcb` text reviewed during the attended 2026-08-23 Pete HIL session

This note preserves what the supplied R23 PCB actually says, what the attached
hardware demonstrated, and which conclusions remain hypotheses. The installed
carrier is R23. R24 does not exist yet; any future R24 is outside this audit.

## R23 routed identities

| Function | Pico identity | R23 carrier path |
|---|---:|---|
| Create command TX | GP0, physical pin 1 | `PICO_TX_3V3` -> TXS0108E LV4/HV4 -> `CREATE_RXD_5V` -> cargo-bay pin 1 |
| Create response RX | GP1, physical pin 2 | cargo-bay pin 2 -> `CREATE_TXD_5V` -> TXS0108E HV5/LV5 -> `PICO_RX_3V3` |
| IMU SDA | GP2, physical pin 4 | `IMU_SDA` -> GY-521 pad 4 and OLED pad 4 |
| IMU SCL | GP3, physical pin 5 | `IMU_SCL` -> GY-521 pad 3 and OLED pad 3 |
| Create power toggle | GP18, physical pin 24 | TXS0108E LV7/HV7 -> cargo-bay pin 3 |
| Translator enable | GP19, physical pin 25 | `TXS_OE_CTRL` -> TXS0108E OE, with 10 kohm pull-down to ground |
| Charging indication | GP17, physical pin 22 | cargo-bay pin 13 -> TXS0108E HV8/LV8 -> `CHARGING_3V3` |

The R23 GY-521 socket routes pad 1 to `+3V3`, pad 2 to ground, pad 3 to
SCL, pad 4 to SDA, and pad 7 (AD0) to ground. If the installed module follows
the footprint, its selected I2C address is therefore `0x68`.

The current Conduit firmware uses the same R23 GPIO assignments: UART0 on
GP0/GP1, I2C1 on GP2/GP3, power toggle on GP18, translator OE on GP19, and the
active-high charging input on GP17. A pin-number mismatch does not explain the
observed symptoms on the installed R23 carrier.

## Findings and risks

### I2C has no carrier pull-ups

The supplied R23 PCB contains no dedicated SDA or SCL pull-up resistors. The
long, branched bus connects both the GY-521 and OLED sockets and therefore
depends on pull-ups inside an installed module or the RP2040's weak internal
pull-ups. Conduit currently enables the RP2040 internal I2C pull-ups, but those
do not establish that the physical bus has suitable rise time or idle voltage.

The attached firmware repeatedly probed both `0x68` and `0x69` at 100 kHz and
reported `device_no_response`, not `identity_mismatch`. The device had power,
but neither candidate address acknowledged. Power indication alone does not
prove adequate MPU VDD, ground continuity, SDA/SCL continuity, or I2C pull-up
strength.

### GY-521 supply must be checked at the sensor

R23 feeds `+3V3` into the GY-521 module's VCC pad. GY-521 module construction
is not fixed by this carrier file; some variants place an onboard 3.3 V
regulator between VCC and the MPU. For such a variant, a 3.3 V carrier input
can leave the actual MPU supply lower than the carrier rail. The MPU-6050
product specification permits VDD from 2.375 V to 3.46 V, so the voltage must
be measured at the MPU-side rail rather than inferred from the module LED or
VCC header.

### The translated UART path is signal-integrity sensitive

R23 routes both UART directions through auto-direction TXS0108E channels and
then across the large carrier and cargo-bay connection. Texas Instruments
documents edge-rate accelerator one-shots in the TXS0108E and recommends short
traces and controlled capacitive loading to avoid retriggering, contention, or
oscillation. The R23 route plus connectors and external wiring is therefore a
material signal-integrity risk even though UART is a supported logical use.

R23 also provides no external idle bias on `PICO_RX_3V3`. The historical
Netherwick firmware enabled the GP1 pull-up because UART idles high; current
Conduit firmware disables both GP1 pulls. That is a firmware/configuration
difference worth testing on R23. It may explain errors
around OE transitions or an undriven interval, but it cannot by itself prove
the cause of errors while the Create is actively driving.

The OE circuit itself is fail-safe in R23: GP19 drives OE and a 10 kohm
pull-down keeps the TXS0108E disabled while the Pico is reset or undriven.

## 2026-08-23 physical evidence

The exact firmware tested was commit `c010837ab5dcfa9f63adddfa0b31b35c2abb67e2`
with UF2 SHA-256
`0c086b263b7a19eac89b5305f5d4f3fd4c67c5e297dc3a44663e989274fab451`.
Its pre-test receipt confirmed OE low, power toggle low, zero UART work, and the
expected Pete USB identity.

One bounded no-motion Full/hello/Safe attempt produced no audible cue and was
refused without wheel authority. After the attempt, the firmware had requested
Safe and returned OE low. Its cumulative UART receipt showed:

- 27 transmitted bytes;
- 22 received bytes;
- 18 framing errors, one break, and one overrun;
- 21 resynchronization-discarded bytes;
- one syntactically valid one-byte sensor response, but no accepted Full mode;
- no checksum-valid mode stream frame.

This is stronger than a generic "no communication" result. Create-directed TX
is reaching far enough to correlate with substantial RX activity, but the
return path is not producing reliable 57,600 baud 8N1 frames at the Pico.

The IMU boot receipt independently reported zero samples and
`device_no_response`. The firmware continues bounded retries, but no successful
probe was observed in this session.

## Required checks before another design conclusion

1. Preserve the exact R23 KiCad PCB and schematic in the hardware-design
   repository and review them as the installed carrier definition. The PCB
   text supplied during this session is the current routing authority.
2. With power removed, continuity-check GP2 to GY-521 SDA, GP3 to GY-521 SCL,
   GY-521 ground, AD0-to-ground, and the intended supply path.
3. With power applied and no Create UART authority, measure SDA and SCL idle
   voltage and rise time, then measure the MPU-side VDD after any module
   regulator. Confirm VDD is at least 2.375 V.
4. Scope Create TX at cargo-bay pin 2, TXS HV5, TXS LV5, and Pico GP1 during
   one attended sensor response. Correlate the first malformed edge with OE
   assertion and Pico framing status.
5. Test the smallest firmware-only UART change on the confirmed R23 mapping:
   restore the GP1 idle-high pull-up used by Netherwick. Keep OE fail-closed
   and use only the bounded no-motion hello entrance.
6. Treat replacement of the auto-direction UART translation with explicit
   unidirectional buffers, suitable level shifting, local decoupling, and
   reviewed termination as a possible future carrier-revision decision, not
   an unreviewed firmware workaround.

## Component references

- [TI TXS0108E product page and data sheet](https://www.ti.com/product/TXS0108E)
- [TDK InvenSense MPU-6000/MPU-6050 product specification](https://invensense.tdk.com/wp-content/uploads/2015/02/MPU-6000-Datasheet1.pdf)
