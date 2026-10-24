# Share immutable composer schema bytes

Core's prepared canonical composer now stores checked byte ranges for names and
child Types inside its one immutable canonical parent Type encoding. It no
longer retains separate owned strings and encodings for those facts. Equal byte
occurrences may share a range: the private parent encoding cannot change, and
runtime child checks still compare the complete exact canonical Type bytes.
The output buffer, preflight refusals and final aggregate validation are unchanged.
The parent prefix still occupies RAM; borrowing linked flash material remains
separate work.

The same 64-bit assigned sampler preparation fixture retains 50,914 bytes rather
than the earlier 52,087 bytes. Both preparation peaks remain 86,750 bytes. This
is a 1,173-byte retained-software reduction, not a measured MCU reduction or a
combined appliance fit. The previous result is preserved in the sampling-value
extent evidence, with the same observation fixture and unchanged backing setup.

All 512 Core tests and 14 appliance/quantity tests pass. The latter include the
full 960-sample workload under seven sensor outcomes and its recurring-allocation
probe. A new Core regression uses field names overlapping child Type identities,
checks owned canonical parity, and rejects swapped Types without destroying the
previous result. Strict Core library Clippy and thumbv6m-none-eabi checking pass.
No firmware was flashed and physical captures remain historical.
