# Finite periodicity observations (development)

This profile measures signal evidence. It does not recover commanded F0,
linguistic identity, or a guaranteed perceptual fundamental. It shares the
existing immutable PCM decoder, content digest, mono layout and exact sample
interval owner. Nothing resamples, pads, aligns or retains hidden block history.

The caller supplies a 64..2048-frame window and inclusive lag range 2..512.
Three complete maximum-lag periods must fit. The workspace contains 2048 f64
samples and 514 f64 correlations; a call performs at most 513*2048 sample-pair
iterations. Construction and reuse allocate nothing. Installed-host work/fuel,
pressure, cancellation and typed output admission remain required before issue
#5686 can close; this portable implementation alone is not normal Play proof.

The finite f64 algorithm subtracts the window mean, then measures normalized
cross-correlation with separate overlap energies. Centered mean-square at or
below 1e-10 digital-full-scale squared is insufficient energy. A local correlation
peak must reach 0.9. Search includes shorter lags outside the requested minimum,
so a clear out-of-profile short period cannot be renamed as its subharmonic.
No qualifying peak yields unknown, including the deterministic noise fixture.

The first qualifying peak is a candidate. A later peak with correlation greater
by 0.0001 is competing evidence, including octave ambiguity from a weak
fundamental beneath a strong second harmonic. A non-repeat peak within 0.03
correlation also yields uncertain. Integer repeats within one sample are not
independent candidate periods. These thresholds are this estimator's declared
profile, not statistical confidence intervals or guarantees on arbitrary audio.

For periodic results the reported lag is an integer sample-grid estimate;
sample_rate/lag is its rational frequency coordinate, not a certified exact
real-world F0. No sub-sample interpolation is claimed. The typed observation carries correlation as integer parts per million,
truncated from the nonnegative f64 result. This declared projection permits
portable integer threshold laws without introducing unsupported F64 expression
operations. Correlation is descriptive agreement, not a probability. Uncertain results retain both competing lags.
Silence/DC, unknown and uncertain remain distinct from a definite periodic result.

Window support is [start_frame, start_frame+frames) on the retained clock and
rate. Its end is the earliest availability coordinate. Observing identical sample
windows in differently sized enclosing PCM blocks gives identical estimates and
intervals but intentionally different source-content digests. This preserves
custody rather than silently equating physically different input values.

The checked output carries a parts-per-million correlation projection. The
`periodicity_score` adapter reuses the generic #4950 score contract with an
explicit estimator/scale and no invented calibration or semantic claim. Actual
Plan/Play Signs establish emission separately. The score does not establish
physical pitch, original speech intent or perception. Generic #4952 fidelity
report integration remains a separate development gate before closure.

Prepared encoding retains four valid canonical state shapes and writes only
admitted numeric cells. The immutable profile fixes source block size, window
offset/length, clock/rate and lag limits. One window is observed per input block;
no cross-block raw history, stitch, padding or inferred resampling is hidden.
