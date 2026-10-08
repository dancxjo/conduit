# Bounded exact quantity trajectories (#5216 prerequisite)

`trajectory_types.conduit` owns the authored Types and separate arithmetic
eligibility; `trajectory.conduit` owns all domain, anchor, order, coverage,
selection and interpolation decisions. Generated native bindings admit complete
frames. Rust traverses the finite sequence, invokes those checked programs and
copies raw result fields into the checked quantity variant; it does no temporal
or interpolation arithmetic.

| Field | Domain / unit | Meaning |
| --- | --- | --- |
| `AudioTimelineIdentity.value` | positive U64 | typed abstract authored timeline identity |
| `AudioOriginIdentity.value` | positive U64 | typed origin identity within that timeline |
| `AudioTrajectoryAnchor.timeline`, `.origin` | those distinct checked Types | exact elapsed-second basis shared by trajectory and query |
| `AudioExactTimeOffset.numerator_seconds` | nonnegative U64 | elapsed seconds numerator relative to anchor |
| `.denominator` | positive U64 | exact divisor, authored fraction retained |
| `AudioTrajectoryQuantity` | frequency / cycle / amplitude / power | existing Audio Hz, seconds-per-cycle, relative amplitude or relative power Types; no implicit interchange |
| `AudioTrajectorySegment.start`, `.end` | exact elapsed seconds | strict positive span, half-open interval except included final endpoint |
| `.left`, `.right` | same quantity domain | independent endpoint values; discontinuities across adjacent segments are permitted |
| `.interpolation` | `step` / `linear` | hold left in interior, or exact rational linear interpolation in the chosen quantity domain |
| `AudioQuantityTrajectory.segments` | 1..16 segments | ordered, internally nonoverlapping; gaps allowed |
| `.outside` | `refuse` | missing coverage, including gaps, refuses; no extrapolation |
| `.endpoints` | `right_continuous_final_included` | shared boundary belongs to next segment; final endpoint returns final right value |
| `.provenance.kind` | authored / imported / measured / derived / unknown | source category, not proof of measurement or authority |
| `.provenance.method` | nonempty Text <=128B | authored source / import / measurement / derivation method identity |
| `.provenance.version` | optional nonempty Text <=64B | retained method/source version when known |
| `AudioTrajectoryQuery.anchor`, `.time` | exact anchor and elapsed seconds | no foreign-anchor conversion is inferred |

These abstract identities establish no relation to a Host clock, boot, ticks,
wall time or sample cadence. Time's monotonic clock contract includes such facts;
Speech's current fractions remain a separate owner. Neither is silently recast.
A future physical/sample projection must retain its explicit clock relationship.
Data's observation provenance requires resource/observation references and does
not represent authored/unknown trajectories, so this bounded Audio provenance
preserves method/version without inventing a measurement source.

A frequency trajectory interpolates in Hz. A cycle trajectory interpolates in
seconds per cycle. Between 100 and 200 Hz, the former yields 150 Hz at midpoint;
between 1/100 and 1/200 seconds per cycle, the latter yields 3/400 seconds per
cycle, whose reciprocal is 400/3 Hz. Log-frequency interpolation is not offered.
Zero amplitude and power are meaningful; zero frequency or cycle duration is
not an unvoiced/unknown state. There is no unresolved quantity variant, and
foreign or unresolved carriers must fail typed admission rather than become zero.

## Separately admitted arithmetic profile

Canonical fractions retain full U64 domains. The first numeric implementation
is explicitly `AudioTrajectoryU8Arithmetic`: each original numerator and
denominator used by a segment/query must be at most 255 (denominators positive).
Preparation admits every segment; evaluation additionally admits the query.
An authored 256/512 fraction refuses this profile even though it equals 1/2.
There is no reduction, rounding, clamping, normalization or approximation.

For start `s`, end `e`, query `q`, Source computes weight numerator
`(q.n*s.d - s.n*q.d)*e.d` and denominator
`(e.n*s.d - s.n*e.d)*q.d`. After Source validates strict span and coverage,
these are nonnegative with positive denominator and weight in [0,1]. For
endpoint fractions `a` and `b`, Source computes linear numerator
`(wd-wn)*a.n*b.d + wn*b.n*a.d` and denominator `wd*a.d*b.d`.
Each weight is below 2^24; the final numerator is below 2^41 and denominator
below 2^40 under this profile. No eligible multiplication can overflow U64.
Source directly returns authored endpoint fractions at endpoints and the left
fraction for step interiors, preserving their unreduced representation.

Coincident start/end and reversed spans refuse. Shared adjacent boundaries are
legal; internal overlaps or reversed segment order refuse. Gaps refuse only
when queried. Independently authored trajectories may overlap and evaluate
separately; no phone/word segmentation or flattened control box is introduced.
All such policy checks execute Source programs. Wrong Types, zero denominators
and out-of-profile operands refuse Native admission before arithmetic. Values
are exact integer fractions, so floating-point NaN/infinity have no admitted
representation. This profile establishes no continuous clock or playback service.

## Inspectable preparation and execution

`PreparedAudioQuantityTrajectory` admits one full canonical trajectory and
retains its source, provenance and exact endpoint fractions. `evaluate` accepts
one complete canonical query. The opaque result exposes borrowed original
trajectory/query frames, selected segment index/frame, every exact Source
program with its actual input/output frames, and the finally admitted semantic
quantity frame. The last blend output is explicitly a raw numeric ratio; Native
admission establishes the semantic quantity afterward. Program evaluation alone
does not establish positive refinements.

A 16-segment trajectory has at most 78 preparation executions and 19 evaluation
executions, 97 total retained executions. The method/version metadata is bounded,
and the sequence bound is checked before traversal. This API allocates during
preparation/evaluation and is a semantic conformance seam, not an admitted Play
Back. There is no runtime scheduling or Flow execution claim.

Measured canonical static Type sizes: trajectory 3454B and query 646B. The
16-segment test carrier is 9479B; its query is 817B. The largest executed
Source input Type is 1978B; that test retains 138532B of executed input/output
frames across 97 expressions (excluding static program references, original
frames and container overhead). These full encodings, not only
numeric fraction payloads, matter for future admission.

Executable native examples/tests cover independent u128 reference values,
step/linear/endpoints, Hz versus cycle interpolation, two independent overlapping
controls, 0.1-second 200-Hz and 1/3-second source fractions, gaps, foreign anchors,
internal overlaps, coincident/reversed spans, profile bounds and forged typed
zero-denominator refusal. Trajectory evaluation itself makes no sample-rate projection claim. The
separate [rate prerequisite](RATE_PROJECTION.md) executes declared sample-grid
and cumulative projections with explicit basis. Actual clock mapping,
log-frequency, additional numeric profiles,
dB/reference conversions, renderer/clock implementations and remaining #5216
acceptance remain open.
