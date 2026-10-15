# Fixed-coefficient band resonator

`dsp/resonator-prepare` takes `ResonatorRequest`: an existing `AudioResonator`
with independent exact center/bandwidth fractions, anchored `AudioSampleRateBasis`,
positive explicit state generation, numeric profile and finite frame count.
It returns a zeroed explicit cursor or a named preparation refusal.
`dsp/resonator-step` accepts that cursor, one signed digital input sample and
cancellation. It returns an advanced cursor/sample, unchanged cancelled or
exhausted cursor, invalid-cursor refusal or overflow refusal.

The `polynomial_q30_to_q14_v1` profile supports rates 8000..192000 Hz, center/rate
from 1/128 through 63/128, bandwidth/rate from 1/256 through 1/8, and 1..65536
frames. Original fractions, anchor, generation and profile remain in the cursor.
The public step has no retuning/interpolation port: changed controls start with
preparation/reset. Checkpoints are explicit state, not authenticated execution
history. Resume preserves the same control request and generation.

The coefficient algorithm uses the finite Q30 constant 3373259426 for pi, nine
integer Taylor updates after the initial unit term for exp(-pi bandwidth/rate)
and cos(2 pi center/rate), then rounds coefficients to signed Q14. Intermediate
terms truncate toward zero; coefficient ties round away from zero. This private
digital approximation is distinct from exact mathematical pi/trigonometric
values and certified numerical enclosures. The projection conformance corpus
checks pole center within rate/8000 Hz and pole bandwidth within rate/20000 Hz.
The recurrence reference is bit-exact. The retained 1 kHz impulse fixture checks
complex frequency response with error at most 0.002 + 0.005 times the analytic
transfer magnitude; these are declared conformance tolerances, not a certified
real-number enclosure for every possible rational control.

For Q=16384, the recurrence is
`y = (sample*Q + b*y1 + c*y2)/Q`, truncating toward zero. Preparation and each
step check `-Q < c < 0` and `-(Q-c) < b < Q-c`, the strict stable-pole conditions
for this offered recurrence. Unsafe later arithmetic uses conditional gates;
portable Boolean operands are eager. Public stepping recomputes the bounded
coefficient projection, so reloaded state never supplies trusted coefficients.
Preparation has 24 ordinary expression Gears and stepping has 27; the polynomial
stage count is fixed, with no hidden runtime loop or planner.

One digital sample unit is 1/32768 of the declared digital full-scale amplitude.
Input samples are -32768..32767; feedback state is signed 32-bit headroom carried
in I128 arithmetic. Results outside that state envelope refuse before storage.
There is no implicit clipping or physical pressure/SPL calibration. PCM limiting,
output devices and any real-time performance claim belong to later composition.

The portable proofs compare actual recurrence samples to an independent Q14
integer reference, compare a complete impulse response to the frequency-domain
transfer function, preserve exact bytes through fresh evaluator partitions and
canonical reloads, and cover reset, generation, cancellation, last-budget and
malformed/extreme/unstable/overflow cases. Every prepared stage checks zero
allocation/reallocation and stable capacity. Separate installed-host proof uses
ordinary Source/Plan/Play and finite queues; retain its actual source-bound
result separately from portable expression conformance.

The shared recurrence preserves compact-voice operation ordering and I32 overflow
behavior. The wider filter profile does not silently widen Speech arithmetic.
Generic speech extraction/parity and whole-parent acceptance remain required
before claiming completion of the relevant DSP issues.
