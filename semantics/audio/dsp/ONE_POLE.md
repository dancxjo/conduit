# Complementary one-pole filters

Development work for #5217. `dsp/one-pole` is ordinary checked Source. It
returns both a low-pass sample and the input-minus-low-pass high-pass sample,
plus the next explicit history and decremented finite frame budget. Reset
uses zero history. Cancellation, invalid profile and exhausted work refuse
without producing a new state. The caller retains its previous cursor.

This is an explicit coefficient-domain profile. The caller owns the admitted
PCM rate; the component introduces no clock and does not accept a Hertz cutoff.
For a fixed Q14 retention `a`, the low-pass recurrence is
`y = trunc((x * (16384-a) + previous * a) / 16384)`. The high-pass output is
`x-y`, symmetrically saturated to -32767..32767. The retained history is the
unsaturated low-pass result, which remains inside that range by convexity.
Both products and their sum fit I32 for the admitted operands. No float,
nonfinite, denormal, device effect or hidden state is involved. Each low-pass
update truncates by less than one sample code. For a fixed pole `a/16384`,
accumulated error relative to the ideal real recurrence is bounded by
`1 / (1-a/16384)` sample codes. High-pass DC rejection therefore includes
quantization bias; it is not an exact zero-DC claim. Saturation can additionally
change the high-pass output when the complementary difference exceeds range.

Retention is in 0..16383; the feedback pole is strictly below one. The coefficient may change explicitly at frame boundaries: every update is
a convex combination of the input and previous state, and truncation toward
zero cannot increase its magnitude. Thus arbitrary admitted coefficient
sequences preserve the same bounded state; this is stronger than checking
only two endpoint poles. Frequency-response claims apply only to a fixed
coefficient. An Hz-to-coefficient projection remains unimplemented. Finite work is 1..65536 frames
per prepared window; a zero budget reports exhaustion. State must be carried unchanged when continuing in a fresh window, with
the authored coefficient sequence retained separately.

This component does not establish physical playback, calibrated pressure,
band-limited oscillation, or completion of the parent DSP issue.
