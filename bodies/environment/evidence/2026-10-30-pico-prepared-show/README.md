# Pico timed display preparation

The Green HT16K33, White Bosch/TM1637 and Yellow DHT11/TM1637 carrier profiles
previously rebuilt a diagnostic semantic Face and resolved quantities on every
redraw. They now prepare the existing common Mask's owned finite Show once per
sample, discard the temporary Face, and select its resolved pages during timed
execution. This reuses `PreparedEnvironmentalShow`; no new formatter or runtime
is introduced. Capture failure clears prepared output, and the existing
30-second lifetime and sensor-specific page selection remain enforced.

## Evidence

- All 13 environmental-Face CPU tests passed, including two tests importing the
  actual Pico display adapter. Temperature 72°F, humidity H 45, pressure 1013 hPa,
  two/three-page ordering, absent BMP humidity, invalid values and expiry checked.
- 30,000 timed selections and actual HT16K33 frame encodings completed while the
  allocator refused every allocation/reallocation. Zero allocations, reallocations
  or deallocations occurred within that execution boundary.
- On the Linux CPU fixture, sample preparation still made 34 allocations with a
  5,200-byte transient peak. The retained owned Show occupied 136 bytes. These
  CPU figures are not Pico runtime heap/stack measurements.
- Strict Clippy passed for the actual adapter test without allowances for its
  logic. The test harness recognizes the firmware's external configuration cfgs.
- Release builds passed for all three checked Pico W profiles with their exact
  feature sets. Existing linker static-memory/stack reservations stayed enabled.
- Green text is 635,900 bytes and BSS 97,224 bytes: +1,604 flash and +72 static RAM
  compared with the shared-layout candidate (634,296 / 97,152). This trades a
  small fixed footprint for removing transient preparation from every redraw;
  it does not establish a reduced preparation peak or a new memory grant.

Raw logs and qualified ELF files are retained locally in
`target/pico-prepared-show-proof`. No device was flashed for this qualification;
Green continues its prior working carrier. Optical behavior and physical heap
qualification remain separate. Carriers still report `body_member:false`; this
change does not claim installed Body sampling/Mask Plays, persistent credentials,
durable history, or Host membership.
