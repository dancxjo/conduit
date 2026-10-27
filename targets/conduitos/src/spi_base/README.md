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
pin-routing, mapping and ordered-access ownership. The machine mechanism supplies no
MMIO constructor, attachment discovery or clock calibration. Generic operation
installation retains independently owned resources before advertising a ready
Back; definitions alone grant no possession or peripheral compatibility. An unexpected
active transfer before exchange signals lost ownership; neither that refusal
nor the owner's subsequent drop overwrites the unknown transfer.

Standing repository validation: `cargo xtask check device-protocols` includes
`device-protocols.spi`. The register-model tests establish finite software
behavior, not a physical SPI waveform or peripheral compatibility. The Mini
PiTFT still needs admitted SPI/GPIO Host Calls, checked ST7789 protocol Source,
normal presentation and button integration, board image selection and physical
boot/interaction evidence.

Register semantics: [BCM2835 ARM Peripherals, chapter 10](https://datasheets.raspberrypi.com/bcm2835/bcm2835-peripherals.pdf).


The generic `machine/spi/exchange` Fore accepts one packed output value of
1–512 bytes. The exact selected native provider owns immutable mode, clock,
chip select and routing; requests carry no attachment selectors. Success returns
exactly the same count of packed input bytes. Canonical schemas retain a
separate 4096-byte Host Call representation bound. Prepared decoding borrows
request storage and prepared result encoding allocates nothing during Play.

`ReadySpiBase` retains actual native ownership before projecting an offer and
consumes that ownership into an exact selected Plan/Play Host Call. The existing
Base capability table enforces authority, operation count and revocation. The
operation factory rejects altered selected contracts. This cooperative boundary
does not confine native code sharing its address space. No BCM2835 register
window is installed by these declarations, and GPIO, protocol execution and
physical image acceptance remain pending under #6515 and #6513.
