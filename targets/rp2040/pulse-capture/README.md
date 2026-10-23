# RP2040 pulse capture realization

This library realizes the portable pulse-read geometry with a consumed GPIO,
PIO state machine, instruction-memory allocation and DMA channel. It recognizes
no peripheral. The composition root chooses those resources; Source supplies
the request's electrical geometry.

PIO times both the low pulse and the sampling offset, while DMA receives
complete octets. The CPU does not mask interrupts for a frame. Completion or
timeout stops the state machine and releases the pin to input. Dropping the
future aborts its DMA transfer and releases the pin through the same guard.
DMA faults quarantine this owner until it is recreated. The integer sample
offset is realized through a finite PIO clock divider; ordinary PIO/input-
synchronizer quantization remains part of the physical realization.

The finite DHT11 diagnostic uses GP2, PIO1/SM0 and DMA2, leaving the onboard
radio's PIO0 and DMA0/1 separate. This is resource separation, not a claim of
verified concurrent Wi-Fi execution.

```sh
cargo xtask prove dht11 build-pio
```

A physical Pico W run captured 32 valid frames through this realization and
evaluated the checked DHT11 Source and generated quantity Forms with allocation
sealed. A subsequent run also used the shared typed pulse-request codec.
These captures prove their preserved diagnostic images; they do not prove
production Plan/Play, radio concurrency, physical timeout/cancellation cases,
other MCU realizations or overnight retention. The constructor consumes native
peripherals; Host installation must additionally bind them to exact admitted
Plan/Play possession before offering this operation.
