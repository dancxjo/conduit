# Fixed numerical DSP profile

The generic fixed-numeric owner provides bounded scalar spectral operations and
explicit numerical conversions. Authored Source chooses windowing, band layout,
sample rate, normalization, waveform and speech policy. The implementation uses
`conduit.numeric/dsp-f64-libm@1`; exact Kind, Fore, artifact and selected Plan
checks remain the existing numeric factory's responsibility.

## Operations

| Kind suffix under `numeric/` | Input → output |
| --- | --- |
| `real-dft320` | 320 finite f32 values → 322 interleaved real/imaginary values, bins 0 through 160 |
| `magnitude-squared161` | 322 interleaved values → 161 squared magnitudes |
| `log10-18`, `log10-1` | 18 or 1 positive finite values → their base-10 logarithms |
| `dot160` | Two 160-element finite vectors → one scalar in a one-element vector |
| `sqrt1` | One nonnegative finite value → its square root |
| `u16-to-f32` | A U16 → its exact value in a one-element finite vector |
| `i16-to-f32-80`, `i16-to-f32-160` | Signed vectors → exact numerical values with the same width |
| `finite-vectorN` for N = 1, 18, 20, 160, 256, 320 | An exact raw-f32 vector envelope → the corresponding checked finite vector |

The forward DFT is unnormalized and uses the negative exponential convention.
DC and Nyquist each occupy one real-spectrum bin. DFT, power and dot calculations
use f64 intermediates and finite f32 results; logarithm and square root use libm.
Independent tests cover impulse/DC/tone, phase shifts, Nyquist and Parseval,
with explicit f32 tolerances. Nonfinite input/output, log-domain and square-root
refusals retain the caller's output and do not consume a kernel input.

Signed conversion preserves the original integer scale. PCM normalization and
physical units remain explicit Source projections. The separate `slice320x40`
Value operation extends the existing index profile: offset 280 is the final
valid window, while overrun, foreign artifacts and pressure refuse.

## Execution and ownership

Value contracts enter through `install_fixed_numeric_catalogs`; the explicit
closing-Flow variants enter through `install_fixed_dsp_flow_catalogs`. A caller
also supplies matching Host offers and selects them in an exact Plan. The
existing `FixedNumericOperationFactory` prepares those selected operations.

Preparation reserves codecs, canonical templates and output buffers. An output
proposal becomes a committed frame only at kernel ACK. Closing Flow drains
queued values before EOF, permits empty streams, and refuses unmatched binary
tails. Pressure or cancellation cannot fabricate output or commit a staged
proposal. The bounded frame counter survives Value reuse and refuses exhaustion.

Reusable Value owners preflight read-only state and rearm only after committed
output; rearm retains code, codecs, selection and cumulative counters. Cancellation
and closing Flow cannot become a new Value invocation. The existing Host/session
owner still admits Plan/Play identity, new inputs and external output acceptance.

Worker preparation reports the Box's inline state through `PreparedWorkerBack`
and all additional codec allocations through the numeric owner. Conformance
measures actual retained allocations for all 15 DSP/conversion operations in
both temporal modes. Shared resources and worker stack admission keep their
existing separate contracts. Slice uses the existing index profile's eligibility.

## Validation and scope

Run the supported repository entrance:

```sh
cargo +stable xtask check fixed-numeric-dsp
```

This checks the numerical references, selected ordinary factory, actual lowered
kernel Cords, EOF/pressure/cancellation, allocation-free Value reuse, retained
worker backing and index bounds. The catalogue comparison separately verifies
that all 93 prior fixed-numeric contracts remain unchanged and the intended
16 Value contracts are added.

This is CPU numerical/kernel proof. Microphone providers, device playback,
phonological intent, model quality, speech tutorials, native timing/containment
and publication retain their own acceptance requirements.
