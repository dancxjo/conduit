# PCM spectrum analysis

`measure` accepts one 64-frame mono PCM block on logical clock 7 at 8 kHz.
The ordinary std Back emits three 32-frame periodic-Hann FFT observations,
with stride 16 and exact frequency spacing 250 Hz. Each row retains the whole
input digest, original source interval, window timestamp and estimator provenance.

The bounded profile supports power-of-two windows 16–1024, at most 16 fitting
windows per block and 1–64 admitted blocks. DC and Nyquist are not doubled.
Peak amplitude uses coherent-gain normalization. Mean-square contribution uses
one-sided window-energy normalization. Both are approximate digital-full-scale
statistics; they make no F0, physical watts, PSD or certified-real claim.

Output envelopes and FFT scratch are prepared before Play. Pressure retains the
input until the last row; cancellation discards remaining work. Invalid whole
PCM refuses before any row. No padding, resampling or cross-block history occurs.

Run `cargo xtask check plots run --deterministic --plot audio-spectrum-analysis`.
This is synthetic PCM conformance through checked Source, ordinary planning and
the production kernel; it is not a device acquisition or listening receipt.
