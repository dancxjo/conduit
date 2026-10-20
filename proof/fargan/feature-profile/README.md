# Public analysis-coefficient fixture

Run `cargo +stable xtask check fargan-feature-frontend` to regenerate the three
pinned coefficient arrays and run the checked Source/generic DSP comparison.
Python's standard library generates the arrays; no download or model framework
is involved. The small public arrays in `coefficients/` are checked in so ordinary
clean-checkout tests need no generation step. Reproduction outputs in `generated/`
are disposable and ignored. Both copies must match the frozen manifest. Every array's shape,
byte count and SHA-256 must match `manifest.json` before the test uses it.

This profile is a proposed spectral approximation from synthetic signed PCM:
causal interpolation, preemphasis, windowing, a real DFT, triangular band
aggregation and a DCT. The independent test equations check preprocessing,
complex phase, power, bands and all twenty output features with explicit
tolerances. The test exercises the ordinary generic numerical execution path.

This fixture does not establish neural FARGAN synthesis, canonical speech-carrier
correspondence, warm state, atomic frame publication, speech quality or physical
playback. It contains public mathematical analysis coefficients, not trained
voice weights. Source admission failures remain failures even when the
coefficient generator succeeds.
