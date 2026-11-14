# Portable HT16K33 packets

`initialize`, `write_frame`, `set_brightness` and `set_display` borrow the caller's
exclusive `embedded_hal_async::i2c::I2c`. They allocate nothing and never create
another peripheral owner. Address is checked to0x70..0x77, brightness to0..15;
blink is an enum. Each whole future needs the caller's deadline. Errors and
cancellation are not success; reinitialize after interrupted initialization.
RAM writes may partially reach the physical device on a bus fault.

Commands and16-byte RAM layout follow Holtek's HT16K33/HT16K33A datasheets:
https://www.holtek.com/webapi/116711/HT16K33Av110.pdf
https://cdn.sparkfun.com/assets/3/f/8/6/2/HT16K33v120.pdf

The finite alpha14 glyph subset is adapted from Adafruit_LED_Backpack's
`alphafonttable` (MIT, retained LICENSE-ADAFRUIT):
https://github.com/adafruit/Adafruit_LED_Backpack/blob/master/Adafruit_LEDBackpack.cpp
The degree glyph is an explicit upper loop. The profile maps logical segments,
DP and four positions to RAM; `adafruit_alpha4` is a selected wiring profile,
not a claim that every HT16K33 backpack has that wiring. Unsupported glyphs and
aliasing/out-of-range wiring refuse. No environmental number formatting lives
here; the common Face/Look remains its owner.

For BME280/BMP280 on the same physical bus, retain `Sensor<I>` and use its
exclusive `bus_mut()` borrow for display transactions between sensor reads.
Calibration/readiness survive this borrow. The old TM1637 target/default is
unchanged. These driver tests prove protocol/encoding, not optical output,
board voltage compatibility, native Body membership or assigned execution.
