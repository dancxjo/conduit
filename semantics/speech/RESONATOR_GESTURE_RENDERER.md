# Explicit-window Source DSP capability

This additive component prepares the existing Speech Source frame renderer from
opaque `PreparedDeclaredPhoneGestures`. Its versioned input profile remains
`speech/authored-acoustic-gesture-demo/1`: complete IPA strings i, a, u, t, d,
tʰ, and t͡ʃ. It does not supply contextual/allophone authority. The caller retains
the opaque choice and joined common-IPA owner separately. It does not cover a
Hello/Travis greeting, general IPA, lateral/rhotic/nasal effects, whole utterance
composition, an actual clock mapping, Flow execution, or a physical audio device.

## Units and coefficient computation

`SpeechResonatorProjectionRequest` retains the original Audio resonator, its
independent center and bandwidth Hz fractions, declared rate, and named profile.
The canonical Audio quantities keep their full U64 domains. Separately admitted
numerical eligibility supports the original denominator 1 only, rates 8000,
16000, 48000, positive center at most 0.45 times rate, and bandwidth from 10 Hz
through rate/8. Reducible authored fractions with another denominator refuse;
no normalization or substitution occurs.

Source computes θ=2πF/rate and x=πBW/rate in Q20, using the declared rounded
constants 6588397 and 3294199. It executes eight recurrence steps for cos θ
(degree 16) and exp(−x) (degree 8), then b=2 exp(−x) cos θ and c=−exp(−2x).
Every integer division truncates toward zero. The final Q20→Q14 conversion
rounds nearest with ties away from zero. The adapter performs exact field copies,
Native admission and bounded traversal; it does not compute coefficients.

Eligibility bounds every multiply: angle square is below 8·2²⁰; polynomial
term magnitude is bounded by 4·2²⁰, making its largest product below 2⁴⁶.
Every raw state is admitted before the next Source step. Final Native admission
requires strict Jury stability and a negative discriminant, preserving a stable
complex pole pair in the existing Q14 recurrence. Quantization that cannot
preserve these conditions refuses with the original request, raw result, every
program/executed frame and admitted intermediate state retained. It never clamps.

The BW parameter declares this pole-decay relationship. It is not an exact
measured acoustic half-power bandwidth or vocal-tract model. Independent tests
compare supported grid cases with f64 exp/cos references and test actual Source
filter transitions against independent i128 recurrence arithmetic. Grid accuracy
measurements are test evidence, not a general transcendental accuracy guarantee.
The actual Core projection report records finite-series/internal/final precision
loss and requires `SpeechQ20Series8Q14Policy`; `require_exact` refuses.

## Timing and DSP entrance

All original gesture windows are projected as absolute elapsed-second endpoints
through the existing exact Audio declared-rate/floor projection. Receipts retain
original fractions, anchor/rate/policy, quotient/remainder, exact programs and
raw/admitted outputs. Endpoint precision loss remains explicit in the existing
Audio Core fidelity reports. There is no implicit sample clock mapping.

Full-size formant values are evaluated through the actual Audio full-U64 step
capability; the separate bounded linear capability is unchanged. Each center and
bandwidth enters the independent coefficient preparation. The first cycle
profile admits only exact integer sample periods from 8 through 512 frames;
fractional sample periods refuse instead of accumulating drift. The original
Audio cycle carrier and sample projection are both retained.

Source evaluates half-open closure/release/aspiration/frication/voicing windows
on the declared integer grid. The new `speech/gesture-controlled-frame` entrance
calls the existing actual Source frame graph, excitation, noise, three parallel
resonators, mix and limiters. It bypasses the legacy `trajectory-release` policy;
that legacy path keeps its 96-frame default unchanged. Closure gates output;
release and aspiration use filtered broadband turbulence; frication additionally
uses the existing 64/256 broadband bypass. Simultaneous turbulence roles refuse
because this profile has one stream. These are explicit approximate effects,
not claims of realistic independent aerodynamic/spectral models.

Each segment starts with declared reset state and fixed coefficients. This is
not cross-segment coarticulation, continuous modulation, or filter-state carry
across coefficient changes. The bounded adapter admits an absolute endpoint no
greater than 48000 frames. Cursor ownership prevents use with another plan.
Every frame retains Native admitted full DSP input/output canonical frames,
exact fixed graph programs and Source gate receipts. Portable graph replay
checks the actual generated DSP output against those canonical frames.

The test WAV contains newly executed Source DSP component contrasts, in order
**i, a, u, t, d, tʰ, t͡ʃ**, each 0.1 seconds at 8 kHz with an original 200 Hz
cycle. It is a component fixture, not the final opaque common-carrier greeting.
