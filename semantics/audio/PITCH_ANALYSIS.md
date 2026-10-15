# Finite periodicity observations (development)

This profile measures signal evidence. It does not recover commanded F0,
linguistic identity, or a guaranteed perceptual fundamental. It shares the
existing immutable PCM decoder, content digest, mono layout and exact sample
interval owner. Nothing resamples, pads, aligns or retains hidden block history.

The caller supplies a 64..2048-frame window and inclusive lag range 2..512.
Three complete maximum-lag periods must fit. The workspace contains 2048 f64
samples and 514 f64 correlations; a call performs at most 513*2048 sample-pair
iterations. Construction and reuse allocate nothing. The installed std Back admits one window per immutable PCM block and 1..64
blocks per Play. Pressure preserves input and budget before analysis; publishing
one observation consumes one block. Cancellation and closing input add no
observation. Final combined-source proof and candidate CI remain closure gates.

The finite f64 algorithm subtracts the window mean, then measures normalized
cross-correlation with separate overlap energies. Centered mean-square at or
below 1e-10 digital-full-scale squared is unknown with an explicit insufficient-energy reason. A local correlation
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
Silence/DC unknown-with-reason, other unknown and uncertain remain distinct from a definite periodic result.

Window support is [start_frame, start_frame+frames) on the retained clock and
rate. Complete-block validation makes the source block end the earliest media
availability coordinate, even when the selected window ends earlier. Source
start/count and this exact availability coordinate are retained separately;
this is not a wall-clock compute-time or physical arrival-time claim. Observing identical sample
windows in differently sized enclosing PCM blocks gives identical estimates and
intervals but intentionally different source-content digests and, when their
block ends differ, different availability coordinates. This preserves
custody rather than silently equating physically different input values.

The checked output carries a parts-per-million correlation projection. The
`periodicity_score` adapter reuses the generic #4950 score contract with an
explicit estimator/scale and no invented calibration or semantic claim. Actual
Plan/Play Signs establish emission separately. The score does not establish
physical pitch, original speech intent or perception. The #4952 fidelity binding checks the complete actual emitted observation
against the retained PCM/profile pair during preparation. It retains the
observation as native evidence and declares waveform reduction as cardinality
loss; strict consumers reject it. The normal Host proof correlates the report
with the actual selected Plan/Host/Boot/Back/artifact and completed attempt.
The report itself does not attest execution; kernel evidence remains separate.

Prepared encoding retains four valid canonical state shapes and writes only
admitted numeric cells. The immutable profile fixes source block size, window
offset/length, clock/rate and lag limits. One window is observed per input block;
no cross-block raw history, stitch, padding or inferred resampling is hidden.

A separate hosted measurement fixture consumes samples produced by the accepted
Audio-owned authored oscillator preparation/step programs. It declares an exact
fixture conversion of one unscaled cubic-difference sample to 1/32768 digital
full scale, then measures those PCM bytes through the same normal measurement
Kind. This proves generated-signal reuse, not a complete #5682 streaming synth,
physical playback or subjective voice quality.
