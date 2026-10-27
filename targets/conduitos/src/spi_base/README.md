# SPI machine mechanisms

The BCM2835 SPI0 polling mechanism is a prerequisite of
[#6513](https://github.com/dancxjo/conduit/issues/6513), scoped by
[#6514](https://github.com/dancxjo/conduit/issues/6514).

It supports one exclusively owned controller, chip select 0 or 1, all four
standard clock modes, an exact even hardware divider, and at most 4096 bytes
per full-duplex exchange. The immutable poll budget covers the whole exchange;
it is a work limit, not a calibrated time guarantee. Receive data is drained
for every transmitted byte, including write-only peripherals. Completion
requires all transmitted bytes, all received bytes and a fresh DONE status.
Any transfer timeout stops the controller and permanently revokes this owner.
Invalid request geometry refuses before traffic without revoking possession.
Received storage can contain partial data after a refusal and must not be
interpreted as a completed response.

The unsafe register-window implementation must retain exclusive electrical,
pin-routing, mapping and ordered-access ownership. This module supplies no
MMIO constructor, attachment discovery, clock calibration, Host offer, generic
Host Call integration, peripheral protocol or ready display claim. An unexpected
active transfer before exchange signals lost ownership; neither that refusal
nor the owner's subsequent drop overwrites the unknown transfer.

Standing repository validation: `cargo xtask check device-protocols` includes
`device-protocols.spi`. The register-model tests establish finite software
behavior, not a physical SPI waveform or peripheral compatibility. The Mini
PiTFT still needs admitted SPI/GPIO Host Calls, checked ST7789 protocol Source,
normal presentation and button integration, board image selection and physical
boot/interaction evidence.

Register semantics: [BCM2835 ARM Peripherals, chapter 10](https://datasheets.raspberrypi.com/bcm2835/bcm2835-peripherals.pdf).
