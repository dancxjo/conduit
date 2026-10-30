# Purpose-selected microphone admission measurement

The fresh thin StdHost constructor plus the actual ALSA discovery/initialization
adapter produces a complete 3,644-byte advertisement. The signed routed request
is 3,318 bytes under the original v2 limits. The reference constructor produces
920,759 bytes and correctly refuses. Whole time, input and native-speech families
also retain their complete metadata and refuse; the text family fits.

`targets/std/tests/purpose_admission.rs` uses an explicitly named discovery-only
executable fixture that refuses capture. It checks provider readiness/provenance,
finite endpoint capacity, complete lossless round trip, exact Host/Boot/generation
scope and replay refusal. It never filters the reference advertisement.

These are CPU construction/codec/signature results, not physical capture, native
Root admission, execution grants, a protected Line or a shared Body Plan/Play.
The incoming finite-Flow adapter adds another initialized offer and must be
measured separately after integration. No memory or wire limit was enlarged.

The installed startup path can request its existing baseline with the checked
Host declaration `software = {capabilities: []}`. Omission retains legacy
composition and identity. Nonempty software choices are checked by Make but
currently refuse installed construction before provider initialization.

Raw test output is retained outside source in the task evidence archive
`raw-audio-admission-20261030/measurements.log` (1130 bytes,
SHA-256 `53f5e70b135bc99e3defc215da686a3dc244e3992fe08af09f2f7248b5687eca`).
