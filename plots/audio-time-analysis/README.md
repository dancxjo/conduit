# PCM time analysis

`measure` accepts one four-frame mono PCM window on logical clock 7 at 8 kHz.
It returns a checked `AudioTimeAnalysisObservation`. The std Host offers the
ordinary `audio/measure-time` Back; the Plot chooses no device or provider.

The observation retains the full canonical input digest, clock, exact half-open
frame interval, rectangular window and digital-full-scale reference. Waveform
pages preserve every selected sample. Mean-square and RMS use normalized digital
amplitude; square-sum is amplitude squared times sample count. These are floating
observations, not physical watts/joules or exact real-arithmetic certificates.
No resampling, trimming, padding, filtering or alignment occurs.

The implementation supports signed 16/24-bit little-endian and float32 PCM,
with windows of 2–2048 frames. Stereo, discontinuity, wrong clock/rate, nonfinite
samples, short input and overflowing frame indices refuse. The output envelope
and sample offsets are prepared before Play, with no runtime allocation in the
analyzer/encoder. Each Back consumes at most its admitted 1–64 blocks. This
fixture admits one block. The enclosing analysis/watch pressure, gaps and
cross-block timing requirements remain with #5223.

Repository proof enters through `cargo xtask check plots run --deterministic --plot audio-time-analysis`.
The proof uses actual checked Source, ordinary planning, std Back and production
kernel. It is synthetic PCM conformance, without device or listening claims.
