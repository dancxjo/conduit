# Bounded audio processing in standing patches

Conduit audio processing is a reusable media-domain contract family. It uses
the existing `conduit.media/audio-frame`, event, control, explicit clock,
bounded cord, exact-plan, lifecycle, workload, and evidence boundaries. It is
not a second scheduler, a command pipeline, a device API, or a collection of
nodes for individual arithmetic operations.

## Current deterministic PCM profile

The checked reference provider implements
`pcm-s16-q15-round-nearest-away-saturate-no-nan-no-denormal-bit-exact`.
Samples are signed 16-bit PCM, gains and matrices are Q15 integers, products
round to nearest away from zero, and outputs saturate to S16. Integers have no
NaN, infinity, or denormal representation; F32 and every other undeclared
format fail resolution or decoding. The reference provider is bit-exact across
supported hosts. An optimized implementation must publish a different
implementation and artifact identity and meet its declared tolerance; an
optimization name never changes the semantic contract.

The first provider supports named `mono-center`, `stereo-lr`, and `stereo-rl`
layouts, no more than two channels, 32 frames or 64 samples per value, two mix
inputs, two gain automation points, one meter side output, and 256 arithmetic
work units per step. Every cord separately declares item, value-byte,
queue-byte, watermark, and pressure bounds. The exact source identity seals
these semantic configuration values; resolution validates them before the
exact plan binds the provider and its finite cord and host allocation.

## Node contracts

- `conduit.media/audio/tee` makes coupled audio fan-out and its two bounded
  output cords explicit before a frame enters multiple processing branches.
- `conduit.media/audio/mix` has the finite named inputs `left` and `right`.
  It waits for one value from each input, then requires the same absolute start
  frame, frame count, sample rate, named layout/order, and discontinuity state.
  Arrival and scheduler wake order do not select a winner. Missing or late
  terminal input fails. Each input has an exact Q15 gain; the sum saturates to
  S16 with no separate headroom. Output keeps the aligned named input layout.
  There is no lookahead, retained sample, or terminal drain in the current
  zero-latency profile; all four facts are exact configuration.
- `conduit.media/audio/gain` applies one exact two-point linear Q15 ramp on the
  absolute input-frame timeline. Chunk boundaries do not restart or shift the
  ramp. The two points are the complete automation allocation. A discontinuity
  retains its explicit timestamp and does not create a hidden ramp origin. The
  reference profile declares zero retained samples.
- `conduit.media/audio/channel-map` requires the input and output channel names
  and an exact supported matrix identity. Mono/stereo conversion and LR/RL
  ordering are matrix operations; equal channel counts never imply
  compatibility.
- `conduit.media/audio/resample` supports exact 24 kHz and 48 kHz ratios with
  the `nearest-hold-bit-exact` profile. It maps timestamps on the absolute
  rational sample grid, has zero group delay and zero retained history, and
  therefore emits nothing on flush. Rate drift or discontinuity is rejected.
  No universal quality flag exists.
- `conduit.media/audio/trim` uses an absolute input-frame, half-open interval.
  Start rounds upward and end downward to the input frame grid. The end may be
  explicitly open. Finite linear fade-in and fade-out lengths are part of the
  exact configuration, and the configured policy preserves a discontinuity
  only when the first input frame is retained.
- `conduit.media/audio/meter` computes absolute S16 peak and integer-square-root
  RMS over one exact input-frame window. Window and cadence are equal, latency
  and retained samples are zero, and exactly one bounded text side value is
  produced. Observation never changes audio delivery or pressure.
- `conduit.media/audio/from-control` is the explicit adapter used by the
  standing-patch proof. One bounded control value becomes one finite stereo
  PCM fixture frame. It does not claim device output, synthesis quality, or an
  implicit control/audio coercion.

Every processing node declares `finite` or `standing` lifecycle in source.
Finite providers complete after the exact fixture value. Standing providers
remain live while their required streams and clock progress. An empty ready
queue yields `Waiting`; only explicit termination, cancellation, failure, or a
lifecycle transition ends the run.

## Composition and evidence

`examples/audio-standing-patch.panel` is a standing graph: an explicit ticker
feeds one sequencer, a bounded slew makes modulation explicit, a register makes
retained control state explicit, and the control-to-PCM adapter feeds a coupled
audio tee. Its two named outputs feed the mixer, then gain, resample, and meter
operate continuously. All cords are capacity one with blocking pressure.
Patchbay Watch attaches to the meter side-output cord and projects the same
ordered authoritative values and timestamps as the textual event table. The
browser may edit permitted source configuration and rerun; it does not perform
DSP, resampling, mixing, metering, pressure, or lifecycle transitions.

Real-time and low-latency are not properties of these contracts. The reference
profile states semantic zero group delay and zero meter latency only. A host
may claim scheduling deadlines or real-time service solely from an admitted
workload guarantee or clearly classified observation under the workload
contract; this provider makes no such claim.

## Required failure distinctions

The current fixture matrix covers silence, two inputs, a silent second input,
late or missing input, sample-rate and channel-order mismatches, clipping,
headroom, a gain ramp split across chunk boundaries, resampler flush, rate
drift, discontinuity, trim rounding and open end, meter cadence, cord pressure,
cancellation, terminal input with no hidden retained sample, and deterministic
versus separately identified optimized tolerance. Unsupported formats,
layouts, rates, matrices, numeric profiles, bounds, and providers fail rather
than inserting an adapter or acquiring host capability.

Requirement identifiers:

- AUD-001: equivalent timestamped PCM chunkings normalize bit-exactly;
- AUD-002: mix alignment and arbitration are independent of scheduler wake order;
- AUD-003: exact source and plan bindings expose finite inputs, samples, state, work, outputs, cords, and pressure;
- AUD-004: numeric, clipping, exceptional-value, and reproducibility policy is explicit;
- AUD-005: unsupported format, layout, rate, matrix, and provider combinations fail closed;
- AUD-006: real-time claims require workload admission or classified observation evidence;
- AUD-007: mix, gain, resample, and meter compose with explicit clock/control panels in one standing checked patch;
- AUD-008: Waiting is nonterminal and explicit cancellation produces the terminal outcome;
- AUD-009: each exported node has checked standalone and composition Tour coverage with equivalent accessible text.
