# Bounded PCM spectrum and spectrogram analysis

For #5749, under #5223. The public `audio-spectrum-analysis` Plot checks,
plans and runs the ordinary `audio/measure-spectrum` std Back. Its 64-frame
input emits three overlapping 32-frame periodic-Hann observations with stride
16 and exact 250 Hz bin spacing. No device or provider is selected by the Plot.

The admitted profile accepts power-of-two windows 16–1024, source blocks up to
2048 frames, stride 1–2048 and at most 16 fitting windows. Requests exceeding
that geometry refuse rather than truncate. A Back admits 1–64 input blocks.
One canonical Source observation is emitted per window, using a shared exact
Frequency origin/spacing and equally sized dense amplitude/mean-square arrays.
The original whole-block digest, clock, source interval, window interval/index,
stride, sample rate, window, reference and estimator remain attached to each row.

The finite radix-2 FFT uses caller-owned scratch. Peak magnitude uses coherent
gain; mean-square contributions use window-energy normalization. Interior bins
are doubled; DC and Nyquist are not. These are approximate digital-full-scale
statistics, not physical watts, PSD or certified-real results. They do not infer
original F0. Rectangular and periodic Hann are the two admitted windows.

Output slots, exact Quantity/schema cells and FFT scratch are prepared before
Play. Encoding only patches admitted digest/timestamp/index and finite numeric
payload cells. Each input stays queued until its last row; output pressure does
not advance the row or block budget. Cancellation prevents remaining output;
closure adds no observation. No hidden buffering, padding, resampling or
cross-block history is introduced.

Independent direct-DFT sine/noise/impulse fixtures, known DC/Nyquist/Parseval
checks, exact fractional-Hz coordinates, checked Source projection and canonical
Native parity cover the numerical/typed boundary. Negative fixtures include
unsupported resolution/window/stride/count, wrong clock/rate, discontinuity,
nonfinite samples outside the selected window, frame/FFT overflow, false axis
units/timestamps and invalid bin queries. Prepared encoding and analysis/bin
iteration have zero-allocation conformance checks. The production-kernel proof
checks complete row cardinality and unchanged admitted value capacities.

Validation at `b104dce2f5de09a8225ca62c2fab1716e82bfaeb`, after accepted
`df66968dec9cdf6eddb813857bf617fd4c3326dd`: all 116 Audio tests, two production runtime tests,
two isolated pressure/cancel/budget tests and the exact Host inventory test pass.
The checked catalog test and public deterministic command also pass. Strict
all-target Clippy for Audio, semantic catalog, std offers and std Host, plus
workspace format, passed at `cbc5536264a14db86a0075fd948bd6c96ad67a94`. Every changed
implementation file is byte-identical between that lint source and the final
combined-development test source; the evidence file records those blobs and
individual execution pins. The reviewed Plot bound is exactly 127 subjects.

Run `cargo xtask check plots run --deterministic --plot audio-spectrum-analysis`.
This is deterministic synthetic PCM conformance, not physical acquisition,
listening proof or parent #5223 completion. No pitch/voicing work from #5686 is
included.
