# Native speech plots

`voice.conduit` owns the compact English voice's typed phoneme-to-phone
realization, acoustic targets, excitation, noise, pitch, resonators, envelope,
limiting, and boundary pauses. Explicit inputs carry state; no clock, device,
provider, or transport is authored here. The source is compiled into a
fixed-storage Back by `semantics/speech`.

Run `cargo xtask prove journey native-speech` for early WAVs and conformance.
See [native segment contracts and proof limits](../../semantics/speech/README.md).
This is an initial phoneme-driven voice, not arbitrary-text TTS acceptance.
