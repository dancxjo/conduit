# RP2040 TM1637 provider

`PicoTm1637` consumes the selected CLK/DIO pin pair and implements the portable
wire transport's open-drain interface. Input mode releases each line; output
mode drives a permanently low latch. Internal pulls are disabled. Drop releases
both lines. No enable GPIO is claimed.

The environmental bench selects GP3/GP4 and external bidirectional 3.3V/5V
translation for the 5V display. The actual shifter is not yet identified.

Timing uses the existing Embassy monotonic driver, with blocking 5 microsecond
half-periods. This is a finite Host operation, not a scheduler. Interrupts may
extend these periods; complete Host admission and physical timing measurements
remain required. This crate is not installed in the production Host and has
not been flashed or physically tested.

Run `cargo xtask prove environment display-target` for the ARM compilation check.
