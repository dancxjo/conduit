# Bounded electrical pulse read

`machine/pulse/read` is a class-neutral Fore. Its exact request/result Types
come from `plots/device-protocols/pulse-types.conduit`; they are checked on the
build host and embedded as canonical Types. This definition does not install
an available Back or establish possession of a pin.

The request supplies a low drive, a number of high pulses to skip, a sample
offset after each following rising edge, an octet count, and a capture deadline.
Samples pack MSB first. Timing coordinates are integer microseconds in the
selected electrical Base's clock. Bounds describe admitted request geometry;
the deadline span is not a promise of zero scheduler or electrical jitter.

`CheckedPulseRead` proves geometry against a finite implementation envelope.
It does not grant resource access. `PreparedPulseCodec` accepts only the exact
request Type and encodes completed, timed-out, provider-failed and geometry-
refused outcomes. A timeout preserves complete octets, with explicit actual
length and zero padding. Partial ISR bits are not published. Empty successful
captures and dirty padding refuse.

The DHT11 supplies its geometry through `dht11-capture.conduit`. Its frame
decoder and quantity Forms remain separate downstream meaning. This mechanism
contains no sensor recognition, checksum, calibration, retry, cadence, units
policy or GPIO assignment.

```sh
cargo xtask prove environment pulse
```

This checks geometry refusals, exact canonical codecs, allocation-free reuse,
and actual Source composition over the Fore. The RP2040 PIO/DMA realization
has separate physical evidence. Runtime Host installation, exact Plan/Play
possession and the ordinary Host Call boundary are still required before this
becomes an installed logger capability.
