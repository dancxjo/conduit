# Exact acoustic quantity prerequisite (#5216)

`types.conduit` owns these renderer-independent quantities. Native Rust carriers
are generated from that Source; `acoustic_quantities.conduit` owns the executed
reciprocal conversions. This is an initial prerequisite, not all of #5216.

| Type / field | Domain | Unit and interpretation |
| --- | --- | --- |
| `AudioFrequencyHz.numerator_hz` | positive U64 | numerator of cycles per second |
| `AudioFrequencyHz.denominator` | positive U64 | dimensionless divisor |
| `AudioCycleDuration.numerator_seconds` | positive U64 | numerator of seconds per cycle |
| `AudioCycleDuration.denominator` | positive U64 | dimensionless divisor |
| `AudioResonator.center` | `AudioFrequencyHz` | independent center frequency |
| `AudioResonator.bandwidth` | `AudioFrequencyHz` | independent positive bandwidth, not Q or a center fraction |

Fractions preserve their authored representation: 400/2 Hz becomes 2/400 seconds
per cycle, then exactly 400/2 Hz again. Both conversions exchange numerator and
denominator, so they require no multiplication, division, floating point or
rounding and cover the full admitted U64 range. Zero numerator or denominator
refuses admission. Unvoiced or unresolved fundamental frequency must use the
existing Speech specification machinery; it is not zero Hz in this Type.

Center and bandwidth have no source-domain Nyquist or sample-rate bound. A
selected realization owns its own eligibility limits. No sample projection,
coefficient generation, filter stability, renderer availability or performance
is implied by constructing a resonator.

Speech retains its existing `SpeechFundamentalCycle` identity. An integration
must explicitly map `numerator_seconds` and `denominator` through both owners'
checked constructors; these Types do not replace Speech's uncertainty or
provenance contracts. No implicit interchange with relative amplitude, power,
dB, probability, gain or checkpoint features is offered.

The focused `acoustic_quantities` native test executes both checked Source plots,
checks unreduced roundtrips and full-range reference cases, and exercises zero
and malformed-input refusals. It reports canonical static Type sizes as well as
nominal numeric payload lengths (16 bytes for each fraction, 32 for a resonator).
Measured canonical static Type encodings are 154 / 161 / 424 bytes for
frequency / cycle / resonator. Canonical frequency and cycle values are
216 / 228 bytes; a canonical resonator value is 576 bytes. Native binding encoding includes structural metadata; it
is not the nominal numeric payload.
These measurements matter because a structured value also carries its static
Type encoding; payload length alone is not its full admission cost.

The bounded trajectory prerequisite is documented in [TRAJECTORIES.md](TRAJECTORIES.md).
Exact declared rate/cumulative projection and its generic fidelity reports are
documented in [RATE_PROJECTION.md](RATE_PROJECTION.md). Reference-bearing dB and its explicitly bounded exact-power-of-ten conversion
are documented in [DECIBELS.md](DECIBELS.md). Physical clock mapping,
control-rate/Q-format projection, catalog realization and remaining #5216
acceptance are deferred. This local semantic proof does not establish playback,
platform execution, model conditioning or stable release acceptance.

The portable expression evaluator is a computation seam, not Type-refinement
admission: generated native constructors/decoders enforce the positive domain
before execution. Integrations must retain that admission boundary. Forged
canonical records with a zero numerator are rejected by those owner decoders;
calling the expression evaluator alone does not establish that refusal.

`PreparedAcousticReciprocal` prepares the exact build-generated Source programs.
Its public methods admit the complete canonical input through the owning Native
decoder, evaluate that original frame, then admit the result through its Native
decoder. Corrupted zero quantities refuse at this composed seam. Preparation
and Native decoding allocate: this API is a semantic preparation/conformance
seam, not an allocation-free Play Back.

Each opaque `AcousticConversion` exposes borrowed access to typed admitted input/result, the unchanged
original canonical input frame, the admitted result frame and exact canonical
Source program hex. Inspection can reproduce the executed conversion without
recovering intent from a rounded numeric output.

## Relative amplitude and power prerequisite

| Type / field | Domain | Unit and reference |
| --- | --- | --- |
| `AudioRelativeAmplitude.numerator` | nonnegative U64 | magnitude numerator relative to a positive matching amplitude reference |
| `AudioRelativeAmplitude.denominator` | positive U64 | dimensionless divisor |
| `AudioPowerRatio.numerator` | nonnegative U64 | power numerator relative to a positive matching power reference |
| `AudioPowerRatio.denominator` | positive U64 | dimensionless divisor |
| `AudioAmplitudePowerRequest.amplitude` | `AudioRelativeAmplitude` | exact authored fraction |
| `AudioAmplitudePowerRequest.relationship` | `same-positive-proportionality` | explicit declaration that signal and reference obey the same positive `P = k A²` relationship |
| `AudioAmplitudePowerEligible.request` | full original request | Source laws require both amplitude fraction fields at most U32 MAX before squaring |

The declaration permits squaring the ratio because the common positive
proportionality cancels. It does not prove measured reference authority,
impedance, absolute SPL or watts. Amplitude is a nonnegative magnitude, distinct
from signed waveform values, gain, probability, power and dB. No logarithm,
reference inference or implicit interchange is offered.

Both quantity Types retain their full U64 domains. Conversion refuses authored
numerators or denominators above U32 MAX even when their ratio is reducible;
zero amplitude is valid, zero denominator is not. Fractions are never normalized:
2/4 amplitude produces 4/16 power. Squared eligible fields fit U64 and a squared
positive denominator remains positive; independent u128 tests check these laws
and their boundaries.

`PreparedAmplitudePower` admits the original full request, then the Source-owned
eligibility wrapper before multiplication. The Source plot returns an explicitly
raw `AudioAmplitudePowerSquared` carrier. The existing refinement construction
guard cannot prove arithmetic-generated positive refinements; the adapter does
no arithmetic and copies the raw fields into the checked `AudioPowerRatio`
constructor. No guard is weakened. The immutable receipt exposes the original
full request frame, eligible executed frame, raw result frame, exact program and
finally admitted semantic result frame as separate identities. This is an
allocating preparation/conformance seam, not a Flow execution or Play Back.

Measured static Type / canonical value bytes: amplitude 157/216, power 150/209,
full request 449/581, eligible wrapper 566/714. Numeric fraction payloads alone
are 16 bytes; the complete structural encodings must be admitted.

This slice adds amplitude-to-power only. Inverse square root and Speech bridging stay open. Bounded trajectories and
explicit-reference exact-power-of-ten dB conversion are implemented separately
as documented above; they do not complete the remaining #5216 criteria.
