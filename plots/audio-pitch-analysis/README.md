# PCM periodicity observations

The checked `measure` Plot consumes mono immutable PCM and emits one typed
`AudioPitchObservation` for the configured window in each block. The std Back
uses the ordinary planner and execution kernel, with no device effect.

The specimen declares four maximum blocks, one 1024-frame window per block,
clock 7 at 8000 Hz, and candidate periods 8..256 samples. The profile retains
exact source digest, window coordinates, algorithm and lag limits. Results
separate insufficient energy, unknown, competing periods and periodic evidence.
An integer-lag estimate is not an exact physical or linguistic fundamental.

Pressure preserves the input and block budget; cancellation or closure creates
no extra observation. There is no hidden stitch, padding, resampling or raw
history. Windows crossing input boundaries require an explicitly prepared
upstream PCM window; this component does not infer such a buffer or schedule.

Repository conformance enters through `cargo xtask check plots`. The inventory
selects the installed Source/Plan/Play proof; portable tests separately check
thresholds, octave ambiguity, window custody and allocation-free encoding.
See the Audio-owned [numeric profile](../../semantics/audio/PITCH_ANALYSIS.md).
