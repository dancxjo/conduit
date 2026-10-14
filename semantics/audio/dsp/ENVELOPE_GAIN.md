# Bounded envelope and gain controls

`envelope.conduit` and `gain.conduit` are checked Source Plots realized by the
existing generic pure-expression Back. Each processes explicit typed controls
and emits a typed result. All arithmetic, bounds, refusal, cancellation-result
and saturation policy are in Source.

Envelope coordinates are integer sample-frame indices, independent of a device
or speech parser. Total length is 1–65,536 frames; frame is 0–total inclusive.
Attack and release are nonnegative, each fits total, and their sum fits total.
Attack rises linearly from zero to Q8 unity (256); sustain holds 256; release
falls to zero at the total endpoint. Integer division truncates toward zero.
Zero attack starts at unity, zero release drops to zero at the endpoint, and
attack plus release equal to total permits a zero-length sustain region.

Gain receives a signed 16-bit integer excitation sample and a dimensionless
Q8 coefficient in 0–4096 (zero through 16×). Its safe wide product is divided
by 256 and saturated to −32767..32767, preserving the compact voice's symmetric
saturation profile. The relative control is not a referenced dB or power value.

Both controls carry an explicit `cancelled` Boolean. Cancellation takes
precedence over profile evaluation and returns `cancelled` with no sample or
level. Other invalid parameters return `invalid_profile` before arithmetic.
These domain outcomes are distinct from Kernel stop/cancellation receipts.
Restart explicitly supplies frame zero and the desired profile. Neither Plot
retains hidden state, so an earlier cancelled call cannot affect a restart.
Callers must distinguish refusal/cancellation from a numeric zero result.

The primitives accept the same numeric control profile for musical and speech
excitation; they impose no pronunciation, oscillator, noise, or mixing policy.
Preparing them requires the existing exact `pure_expression_std_offer` for the
checked program and temporal contract. Ordinary planning retains Source and
selects the generic Back before Kernel execution.

Reference tests cover full envelope curves and endpoints, zero-duration
segments, the maximum frame profile, gain rounding and saturation, extreme
invalid controls, cancellation precedence and reset. Installed std Host tests
check actual requested/completed HostCall Sign events and fixed Kernel value
allocation capacity. These are deterministic hosted execution receipts.
