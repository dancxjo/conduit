# Exact declared sample-grid projection (#5216 prerequisite)

`rate_types.conduit` owns full authored quantities, projection requests, cursor
and result laws. `rate_projection.conduit` owns quotient/remainder, common-basis
comparison, origin initialization, cumulative append and exact-versus-floor
classification. These are declared abstract projections, not clock offers,
PCM/rendering implementations, Flow execution or Play Backs.

| Field | Domain / unit | Meaning |
| --- | --- | --- |
| `AudioTimeFraction.numerator_seconds` | nonnegative U64 | exact duration numerator |
| `.denominator` | positive U64 | exact authored divisor, never reduced implicitly |
| `AudioSampleProjectionQuantity` | duration / cycle | duration above or existing positive `AudioCycleDuration` |
| `AudioSampleRateBasis.anchor` | existing typed timeline/origin | abstract elapsed-second origin; no actual clock mapping |
| `.sample_rate_hz` | positive U64 | declared samples per second, distinct from availability or a renderer's supported rate |
| `.quantization` | `floor` | explicit integer-frame endpoint quantization |
| `AudioSampleProjectionRequest.basis`, `.quantity` | full original carriers | exact source and declared projection context |
| `AudioSampleProjectionRaw.whole_frames` | U64 frames | Source-computed floor of exact duration × rate |
| `.remainder_numerator` | U64 | fractional frame numerator over unchanged source denominator |
| `AudioCumulativeFrameBasis.denominator` | positive U64 | exact common original denominator across appends |
| `AudioCumulativeFrameCursor.whole_frames` | U64 frames | cumulative floor relative to declared origin |
| `.remainder_numerator` | nonnegative U64 < common denominator | retained cumulative fractional-frame carry |
| `AudioCumulativeFrameRaw.frame_count` | U64 frames | new cumulative floor minus previous floor |
| `.whole_frames`, `.remainder_numerator` | endpoint frames and exact carry | cumulative endpoint, not an independently rounded duration |
| `AudioIntegerFrameTarget.basis`, `.whole_frames` | full declared context and integer frame count | frame-grid target used by the generic projection report |
| `AudioSampleProjectionChain.quantities` | 1..16 duration/cycle quantities | finite same-basis cumulative chain |

The full Source quantity, rate and cursor domains remain U64. A separately
admitted arithmetic implementation requires each authored duration/cycle
numerator and denominator at most U32 MAX, rate at most 192000, and a prior
cursor whole-frame count at most 2^63−1. These are eligibility bounds, not a
semantic clock or quantity limit. Reducible out-of-profile fractions refuse;
no clamping, reset, normalization, approximation or denominator conversion occurs.

Standalone Source computes `whole = n*rate/d` and `remainder = (n*rate)%d`.
Cumulative Source first verifies identical declared anchor, rate, policy and
original denominator. It then computes `total = n*rate + previous_remainder`,
`frame_count = total/d`, `next_remainder = total%d` and
`next_whole = previous_whole + frame_count`. The common denominator and Source
cursor law `previous_remainder < d` are admitted before arithmetic.

Under the profile, `n*rate + carry < 2^50`, and adding its quotient to a prior
whole count at most 2^63−1 fits U64. Final native result laws independently check
these exact equations, denominator and full cursor basis. The adapter copies
fields and traverses the finite chain; it performs no projection arithmetic.
An output cursor may be valid in the full semantic domain while exceeding the
next append's conservative eligibility; the next append then refuses explicitly.
A fresh chain starts at Source-authored zero frames/carry, never by silently
resetting an existing cursor.

For three consecutive 1/3-second durations at 8 kHz, spans are 2666, 2667, 2667
frames, cumulative endpoints 2666, 5333, 8000, and carries 2/3, 1/3, 0/3 frame.
Independent flooring would have lost two frames; this projection preserves the
exact cumulative remainder. An authored 0.1-second duration projects to
800/1600/4800 frames at 8/16/48 kHz; the exact cycle derived from authored 200 Hz
projects to 40/80/240 frames. Tests retain identical original quantity carriers
across those declared rates. No physical/sample clock relationship is inferred.

## Exact evidence and #4952 reports

Opaque projection receipts retain the complete original request frame, every
actual checked Source program/input/output frame, raw quotient/remainder,
finally admitted semantic result frame, and the declared integer-grid target.
Cumulative receipts also preserve the original cursor and next cursor; chain
receipts retain the entire finite source sequence and Source origin execution.
The shared `AudioSourceExecution` helper preserves the same execution contract
used by trajectories; the existing trajectory evidence name remains an alias.

Source classifies the integer endpoint as exact when remainder is zero and
floored-with-remainder otherwise. This Audio-specific Source decision feeds the
existing core `ProjectionReport`, `ProjectionFidelity` and `ProjectionLoss`
contracts; there is no competing generic fidelity mechanism.

`AudioFrameGridProjection` borrows an immutable executed receipt. Its complete
obligation inventory requires anchor, declared rate and floor policy to remain
in the integer target. Frame-grid precision is a named exact transformation or
a known `Precision` loss with the exact retained fractional remainder and
original native source frame. `AudioFloorFrameGridPolicy` authorizes only that
known endpoint precision loss. Omitted obligations, fabricated preservation,
other loss classes and changed targets/native evidence refuse. Core exact
consumers reject a correctly reported permitted floor loss.

The report's target is the declared integer grid, not the retained original
fraction or complete receipt. Keeping those originals for audit does not make
an integer endpoint exact. No Plan, Host, Back, implementation attempt or effect
attestation is invented by these semantic reports.

## Bounds and proof boundary

Measured canonical static Type / value bytes: duration 160/227, projection
request 1226/1490, admitted projection result 1870/2348, cumulative request
2323/2891, cumulative result 3975/5074. The largest executed Source input Type
is 2615B. These complete semantic carriers exceed a 2-KiB Type envelope where
that envelope is selected; no bound has been raised and no such Flow admission
is claimed. A future private arithmetic projection may reduce execution Types
while retaining these original semantic identities.

The #4952 native source fact is the original request (2891B in the cumulative
example), within the existing 4096B per-native-fact bound. The larger admitted
result remains separately retained in the receipt; no evidence is truncated or
substituted to force it into that fact.

Native examples/tests execute 8/16/48-kHz projections, the three-thirds chain,
independent u128 references and extreme eligible operands. They also exercise
full-domain-but-out-of-profile values, cursor overflow eligibility, foreign
rate/anchor/denominator, malformed frames, forged carry, incorrect fidelity,
finite sequence bounds and adversarial generic projection reports. Foreign
quantization variants refuse Native admission rather than fall back.

This establishes exact declared rate/cumulative semantics only. Actual clock
relationships, control-rate/Q-format projections, additional numeric profiles,
renderer/PCM eligibility, stable acceptance and all remaining #5216
requirements remain open. The separate explicit-reference exact-power-of-ten
dB capability is documented in [DECIBELS.md](DECIBELS.md); general logarithmic
conversion remains open.
