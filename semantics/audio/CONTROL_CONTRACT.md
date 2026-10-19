# Audio control domains and selected realizations

This map aggregates the generic Audio contracts required by #5216. Source
quantities, declared projection contexts, and digital realization profiles have
separate admission boundaries. Original fractions are never silently normalized
or replaced by a renderer coefficient. A positive frequency is not an unresolved
or unvoiced marker. A declared clock relation is not measured calibration.

## Original controls and ownership

Paths below are relative to this directory unless prefixed otherwise. All
canonical input records must pass their owner Native decoder before execution;
expression evaluation by itself does not establish refinement admission.

| Control | Source owner and domain | Meaning and supported path |
| --- | --- | --- |
| Frequency | `types.conduit::AudioFrequencyHz`: positive U64 numerator Hz / positive U64 denominator | Cycles/second; exact reciprocal in `acoustic_quantities.conduit`, transported by `AudioFrequencyControl` in `exact_control_types.conduit`. No source-domain Nyquist restriction. |
| Cycle | `types.conduit::AudioCycleDuration`: positive U64 seconds numerator / positive U64 denominator | Seconds/cycle; independent semantic domain from Hz even when reciprocally convertible. |
| Center and bandwidth | `types.conduit::AudioResonator`: two independent `AudioFrequencyHz` values | Positive center Hz and positive bandwidth Hz. Bandwidth is not Q, a center fraction, a probability or a filter coefficient. Selected realization below owns its eligibility. |
| Time and anchor | `trajectory_types.conduit::AudioExactTimeOffset`, `AudioTrajectoryAnchor` | Nonnegative U64 seconds numerator / positive denominator; positive typed timeline and origin identities. Anchor equality establishes only the authored abstract basis. |
| Phase | `exact_control_types.conduit::AudioPhaseReference` | Signed I64 turns numerator / positive U64 denominator, exact time, anchor and provenance. Whole authored turns remain retained. `AudioPhaseReferenceMatch` refuses foreign anchors. Periodic realization evolves an explicit finite phase cursor; this authored reference alone does not establish physical phase alignment. |
| Relative amplitude | `types.conduit::AudioRelativeAmplitude`: nonnegative U64 / positive U64 | Magnitude relative to the matching positive amplitude reference. Distinct from signed sample, power, gain and probability. `amplitude_power.conduit` squares only an explicitly declared common positive proportionality, with original operands at most U32 MAX. |
| Relative power | `types.conduit::AudioPowerRatio`: nonnegative U64 / positive U64 | Power relative to matching power reference. No implicit amplitude interchange, inverse square root or watts claim. |
| Referenced level and dB | `decibel_types.conduit::AudioDecibelReference`, `AudioReferencedLevelRatio`, `AudioDecibelValue`, `AudioDecibelBasis` | Explicit magnitude/unit/identity/role/provenance, signed rational finite dB or negative infinity. Amplitude uses 20 log10 and power 10 log10. The exact-power-of-ten profile and refusals are in [DECIBELS.md](DECIBELS.md); no general logarithm is supplied. |
| Quantity trajectory | `trajectory_types.conduit::AudioQuantityTrajectory` | 1..16 ordered nonoverlapping segments in frequency, cycle, amplitude or power; step or exact rational linear policy in that chosen domain. Half-open segments, right-continuous shared boundaries, final endpoint included; gaps/outside refuse. |
| Overlapping controls | `trajectory_types.conduit::AudioControlTrajectorySet` | Up to eight separately identified controls, each bounded trajectory; explicit overlap/priority/tie and gap policy. This does not infer phonetic structure or merge unrelated semantic domains. See [TRAJECTORIES.md](TRAJECTORIES.md) and `src/control_trajectories.rs`. |
| Provenance | Audio trajectory/control provenance | Authored/imported/measured/derived/unknown category, nonempty method at most 128 bytes and optional nonempty version at most 64 bytes. A category called measured does not prove an observation. Full original metadata is retained in preparation receipts. |
| Declared sample grid | `rate_types.conduit::AudioSampleRateBasis` | Positive U64 samples/second, explicit anchor and floor policy. Selected arithmetic: rate at most 192000 and original quantity operands at most U32 MAX. Full request, exact remainder and cumulative carry remain retained. [RATE_PROJECTION.md](RATE_PROJECTION.md). |

Literal admission (`src/exact_controls.rs`) uses checked Core physical quantities
and caller-owned work budgets: `440Hz`, `1.25Hz`, `250ms` preserve source spelling
and exact conversion custody. Wrong dimensions, negative values, exhausted work,
unsupported width and mismatched literal spelling refuse. No default clock,
reference, unit conversion or implicit foreign-anchor conversion is inferred.

## Offered digital controls and profile precision

These are finite digital contracts. They do not redefine the original acoustic
units, establish hardware access or claim arbitrary real-time scheduling.

| Offered control/profile | Executed Source and complete admitted limits | Precision and state |
| --- | --- | --- |
| Periodic source | `dsp/periodic-types.conduit`, `periodic-oscillator.conduit`, `periodic-flow.conduit`; `AudioPeriodicRequest` retains frequency, phase, amplitude, common anchored basis, `unscaled_cubic_flow` reference and 1..65536 frames. Rates 8000..192000; Q8 period 2048..131072; amplitude ratio 0..2. | Checked I128 division yields period Q8 and amplitude Q15 plus exact remainders. Phase normalization is modulo whole turns only in realization; initial phase Q8 and its remainder remain explicit. Cursor retains request, emitted/remaining counts and phase; cancellation/exhaustion are distinct. A nonzero remainder is not an exact integer coefficient merely because original intent remains available. |
| Resonator | `dsp/resonator-types.conduit`, `resonator-projection.conduit`, `resonator.conduit`, `resonator-recurrence.conduit`; positive generation, 1..65536 frames, rates 8000..192000; center/rate 1/128..63/128; bandwidth 10 Hz..rate/8. | `polynomial_q30_to_q14_v1` uses explicit finite pi/Taylor arithmetic and signed Q14 coefficients. Strict pole stability and signed-32 feedback headroom gate execution; overflow refuses. Original independent Hz fractions remain retained. Corpus tolerances and rounding rules are in [dsp/RESONATOR.md](dsp/RESONATOR.md); these are conformance tolerances, not a universal certified real-number enclosure. |
| Gain | `dsp/gain.conduit::GainControl`: signed16 sample, gain_q8 0..4096, explicit cancellation. | Dimensionless multiplier gain_q8/256, truncation toward zero and symmetric saturation ±32767. Gain changes a sample; it is not an amplitude measurement/reference or dB. |
| Envelope | `dsp/envelope.conduit::EnvelopeControl`: total 1..65536 frames, frame 0..total, attack/release nonnegative with sum at most total, explicit cancellation. | Q8 attack/sustain/release level 0..256; integer truncation, final frame zero. Frame coordinates are explicit; no elapsed clock relation is inferred. |
| Deterministic noise | `dsp/seeded-noise.conduit::NoiseStepInput`: word 0..65535 and remaining 1..65536; candidate entry checks all bounds. | LCG next word `(25173*word+13849)%65536`; signed sample next word−32768. State, finite remaining budget, invalid seed and exhaustion are explicit. This is reproducible digital excitation, not entropy, white-noise certification, airflow or a probability. |
| Mixer | `dsp/mixer.conduit::MixerChannel`: signed16 sample, Q8 gain 0..4096, nonnegative timeline, frame 0..65535, rate 1..192000. | Two/four channels must have identical timeline/frame/rate. Each product truncates, then ordered sum saturates once to ±32767; mismatch refuses. Numeric equality does not establish physical clock correspondence. |
| Voicing, aspiration, frication | `dsp/independent-excitation.conduit::IndependentExcitationInput`: three independent `SharedExcitationControl` cursors from `dsp/experimental-q14-frame.conduit`. | Voiced cursor must select periodic excitation; aspiration and frication must select nonperiodic excitation. Their independent explicit words/budgets/filter/envelope state remain separate. The offered specimen mixes at declared 8000 frames/second, timeline 1, gains Q8 128. Labels describe independent digital channels, not inferred continuous voicing probability, vocal-fold state, airflow or gesture physics. |

`SharedExcitationCursor` carries word, remaining, frame/total, attack/release,
sample, periodic Boolean, period_q8/phase_q8, b/c and y1/y2. Per-step Source
validates period 2048..131072, phase 0..period−1, b ±8192, c ±4096 and feedback
histories ±32767 before its bounded recurrence. Its standard starts declare
160 frames, attack 8, release 16, period Q8 4096, phase zero and zero histories.
The imported noise/envelope/gain operations validate their own remaining fields.
This coefficient-domain experiment is separate from the broader resonator
profile and must not be advertised as a general acoustic parameter inference API.

Spectral tilt is not offered by these profiles. Log-frequency interpolation,
general logarithms, arbitrary interpolation/extrapolation and automatic reference
or physical-clock inference are unavailable. This does not replace the required
positive declared clock/control-rate path described next.

## Clock and control-cadence completion boundary

Data owns `semantics/data/types.conduit::ClockRelation`: relation identity,
source/target clock identities and anchors, positive tick ratio and exact versus
estimated quality/error. Time owns monotonic clock identity, boot/basis, scale,
resolution and uncertainty. Audio sampled-signal mapping retains declared clock,
start and cadence; none of these carriers alone executes an Audio-time conversion.

The declared clock completion Source is `clock_projection_types.conduit` and
`clock_projection.conduit`, with `src/clock_projection.rs` exposing
`PreparedAudioClockBinding::new(...).project(...)`. `AudioClockBinding` retains
an Audio anchor, complete Data relation, explicit source/target clock names,
source origin tick and selected `seconds` source-unit profile. Audio origin must
match the relation's source anchor; names must match its direction. This profile
accepts exact quality only. Original query time and relation tick-ratio operands
are bounded to U32 MAX for execution; the final projected target is bounded U64.
Source computes target-anchor plus the quotient of elapsed seconds × target ticks
/ source ticks, preserving the complete remainder and denominator. This is a
caller-declared affine relationship, with no calibration claim.

The bounded cadence Source is `control_projection_types.conduit` and
`control_projection.conduit`, with `src/control_projection.rs` exposing
`PreparedAudioControlProjection::new(original).project(schedule, clock)`.
`AudioControlProjectionSchedule` retains anchor/start, distinct
`AudioControlCadence.updates_per_second` (1..192000), update_count (1..16) and
`point_samples_only` reconstruction. Original start operands and start denominator
× cadence are bounded U32 MAX. Source computes exact query fractions into
`AudioControlQueryTimeRaw` (numerator and denominator). The Rust adapter copies
those fields with the original anchor into the existing checked
`AudioExactTimeOffset`/`AudioTrajectoryQuery` owner constructors; it performs no
cadence arithmetic. Raw output alone does not establish the positive-denominator
refinement. Receipts retain the original trajectory set/schedule, raw result,
finally admitted query, each evaluated control and clock receipt,
program/input/output and separate final boundary query. Point samples establish
no between-point reconstruction or sample-and-hold guarantee. The selected
trajectory step/linear profile still controls numerical eligibility.

Local clock/control validation now passes four clock tests, five cadence tests,
two cadence-report tests, three public catalog tests and four installed-host
Source/Plan/Play tests. These exercise positive clock conversion with nonzero
anchors and three declared rates, 100/200-Hz control queries, retained originals,
finite cadence and refusal paths. The installed tests consume the four reusable
public callers in [examples/clock-control.conduit](examples/clock-control.conduit).
Run the supported aggregate with `cargo +stable xtask check acoustic-control`.
The focused local results do not assert that the final aggregate, exact-head
Integration, publication or whole-parent acceptance has passed.

The preparation catalog implementation is
`semantics/catalog/src/acoustic_control_catalog.rs`, with all five exact
`audio/clock-*` and `audio/control-*-time` Source Fores/Backs and owner Types;
`semantics/catalog/tests/acoustic_control_catalog.rs` checks caller lookup,
expansion, exact Source/plot identity and atomic refusal. Neither the public
example nor these semantic preparation Fores acquires hardware-clock authority.

The new numeric adapters preserve the existing owner algorithms and use Core's
`ProjectionReport`/`ProjectionFidelity`/`ProjectionLoss` contracts:

* Audio's `AudioPeriodicNumericProjection::new(&cursor)` and
  `AudioPeriodicNumericPolicy` expose initial period Q8, amplitude Q15 and phase
  Q8 with exact original request and per-field retained remainders. Advanced
  cursors refuse this initial-projection report. A nonzero remainder is a known
  precision loss; retaining original fractions does not make the numeric target
  exact.
* Catalog's `PreparedResonatorNumericProjection::new().project(input_canonical)`
  executes the existing owner Source's 24-stage resonator preparation chain,
  retains exact source/checked/expanded identities and all execution receipts,
  and requires the actual started outcome. `ResonatorNumericProjection` and
  `ResonatorPolynomialPolicy` describe its original Hz request and Q14 target.
  Polynomial mapping reports `Approximation`, with the existing corpus-only
  center/bandwidth/response tolerances. It supplies no certified per-request
  bound or universal numerical enclosure.

Both numeric adapters pass their four focused tests. The supported aggregate
passes 98 tests across 25 binaries, including the public installed Source example,
periodic and resonator Plan/Play, exact quantities, clocks and control cadence.
The strengthened original-to-PCM custody test also passes separately; production
code is unchanged from the aggregate-tested revision. Exact-head CI and stable
acceptance remain pending. The policies reject
fabricated preservation, missing obligations and changed target/source evidence;
exact consumers must reject a lossy numeric target. The final aggregate must
retain that evidence alongside positive independent control/sample-rate cases
and all required domain, coverage, count and clock-relationship refusals.

## Public surface, proof and accepted component receipts

The existing public catalog implementation is
`semantics/catalog/src/audio_dsp_catalog.rs`, with package/profile ownership in
`audio_dsp_package.rs` and `audio_dsp_profiles.rs`. It exports `audio/dsp/` paths
including oscillator-prepare/step, resonator-prepare/step, gain, envelope,
noise-step-candidate/value, mix-two/four and experimental-independent-excitation. Exact lookup,
Type/source identity and package checks live in
`semantics/catalog/tests/dsp_public_catalog.rs` and `dsp_package.rs`.

| Component | Authoritative source and acceptance receipt | Executable evidence to retain/reuse |
| --- | --- | --- |
| Exact quantity/phase transport | #5673, `e4d605abd64c542dc792af96dd5571f711794158`; [receipt](https://github.com/dancxjo/conduit/issues/5673#issuecomment-6162610583) | `tests/exact_quantity_projection.rs`, `tests/exact_controls.rs`, `targets/std/tests/exact_audio_controls.rs` |
| Exact cumulative sample grid | #5674, `4c8b7a3b882f1986d277d6ae60d63801aeed2287`; [receipt](https://github.com/dancxjo/conduit/issues/5674#issuecomment-6171678103) | `tests/rate_projection.rs`, installed rate projection targets and exact-versus-floor reports |
| Amplitude/power/referenced dB | #5743, `81ce8783ebf98f10662801b38e129d2fef94c596`; [receipt](https://github.com/dancxjo/conduit/issues/5743#issuecomment-6174763947) | `tests/decibel_projection.rs`, independent exponent/reference cases and refusal evidence |
| Bounded trajectories/control overlap | #5745, `a1d9ad1273925229ecd95a2d878aed48879efa55`; [receipt](https://github.com/dancxjo/conduit/issues/5745#issuecomment-6175642396) | Native trajectory/control tests, owner domain/coverage/priority refusals |
| Cumulative integrated numeric contract | #5675, `992cfa831cde7e4ea82de0af9bf78d87e3270ee6`; [receipt](https://github.com/dancxjo/conduit/issues/5675#issuecomment-6217344987) | `cargo +stable xtask check acoustic-control`, installed Source/Plan/Play and projection fidelity |
| Periodic profile | #5676 / #5882, `c5adc5df88596236ef0f5f8ee086e2fa3ce1a085`; [receipt](https://github.com/dancxjo/conduit/issues/5676#issuecomment-6201146192) | `tests/periodic_oscillator.rs`, `periodic_validation_guards.rs`, `targets/std/tests/periodic_oscillator.rs` |
| Resonator profile | #5677 / #5885, `e2e8bb424d6a49d6de877df3788b891502122d56`; [receipt](https://github.com/dancxjo/conduit/issues/5677#issuecomment-6203617638) | `targets/std/tests/resonator.rs`, source-pinned projection/impulse corpus and independent recurrence/response references |
| Gain/noise/independent channels | Existing checked DSP Source and catalog above | `targets/std/tests/envelope_gain.rs`, `seeded_noise.rs`, `dsp_package_frames.rs`, `dsp_shared_excitation.rs` |

Historical receipts establish their stated component scope only. Reuse requires
unchanged relevant code/dependency identities; it does not waive final exact-head
Integration or prove whole-parent acceptance/publication. `ProjectionReport`,
`ProjectionFidelity` and `ProjectionLoss` remain Core owners. Integer targets
with nonzero exact remainder or declared polynomial approximation cannot be
reported lossless merely by retaining originals; exact consumers must reject
rounded-only targets. The final parent aggregate must bind the clock/cadence
positive tests, all offered fields/profiles, public lookup/Type identity and
fidelity/refusal obligations to one accepted revision. Physical device,
calibration, hardware timing, voice training and human listening are not proved
by these semantic/digital contracts.
