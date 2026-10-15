# Bounded PCM time-domain observations

For #5685, `audio/measure-time` observes an immutable mono PCM window through
ordinary checked Source, planning, the selected std Back and the production
kernel. The canonical fixture is `plots/audio-time-analysis/main.conduit`.
The implementation and focused tests are captured at Source
`58055651eceb96c1a00160f28120eceb1ae1d34f`.

The observation retains the digest of the complete canonical input frame,
logical clock, sample rate and exact half-open frame interval. Waveform values
use digital full scale. Mean-square is normalized amplitude squared, RMS is
normalized amplitude, and square-sum is normalized amplitude squared times
sample count. These are descriptive floating projections, without a physical
power/energy or exact-real arithmetic certificate. Summation uses compensated
binary64 arithmetic; input signed 16/24-bit and float32 samples convert exactly
to binary64 before aggregation.

Preparation admits 2–2048 frames at 8–192 kHz. The checked Source observation
has at most two waveform pages, each at most 1024 finite binary64 samples.
Source laws enforce page lengths against the frame count, nonnegative finite
statistics, full-scale count and nonoverflowing sample intervals. The original
frame bytes are borrowed. No padding, resampling, filtering or alignment occurs.

The output encoder prepares a complete canonical Native observation through its
generated binding, then discovers primitive payload offsets through validated
borrowed views. Play patches numeric slots; it cannot change schema, collection
length, window geometry or reference. Its exact retained heap is output bytes
plus one `usize` offset per sample. Tests compare every byte with the generated
binding at 2/1024/1025/2048 samples and count zero allocations/reallocations during
analysis and encoding. Prepared output and kernel value-store capacity remain
fixed. Each Back admits at most 1–64 blocks and reserves output/value/Sign
storage before Play.

Typed preparation refusals distinguish invalid frame, unsupported layout, wrong
timebase, discontinuity, insufficient window, window outside frame, frame-index
overflow and nonfinite sample. The std profile maps these respectively to
`InvalidInput` details 1–8; output specialization mismatch is detail 9. Exceeded
block allowance gives `WorkBudgetExhausted`. Invalid input produces no fake
observation. Stereo is explicitly unsupported by this first profile.

Validation at the source above: all 103 Audio tests across 24 nonempty targets
passed; catalog Source conformance passed; two production-kernel tests passed
across sine/impulse/silence and invalid clock/rate/nonfinite/short/discontinuous
cases. Actual selected implementation, Plan/Play correlation, mandatory Signs,
canonical output custody and unchanged value-store capacity are asserted.
Strict all-target Clippy for Audio, semantic Catalog, std offers and std Host
passed with warnings denied. The documented xtask command passed both public
Source checking and its declared production-kernel oracle. Its wrapper report
retains Source/checked identities; its Plan/Play fields are null, so actual
Plan/Play custody is proved by the linked kernel test assertions.

The public entrance initially refused the added specimen because the inventory
contained 125 reviewed subjects with a bound of 124. The bound now admits
exactly 125. This is explicit membership, without a generic discovery fallback.
Later ownership, test-assertion and inventory edits leave the measured runtime
files byte-identical to the Source named above.

This is native synthetic-PCM and production-kernel conformance. Device capture,
physical playback, perception/listening, cross-block watch gaps/pressure,
spectral/F0 estimation and parent #5223 completion are separate proof classes.
The selected repository entrance is
`cargo xtask check plots run --deterministic --plot audio-time-analysis`.

## Validated typed-unit ABI

The current observation gives signed sample amplitude, amplitude magnitude,
mean-square power and square-sum distinct nominal checked Types. An explicit
preparation-only borrowed representation view preserves strict nominal admission.
Fresh validation at `5ebec2a50637a619ef8bdc38eea9871a0507596d` passed 108 tests: one Core regression, 104 Audio tests,
one Catalog conformance test and two production-kernel tests. Encoder byte parity
and zero allocation/reallocation passed at 2/1024/1025/2048 frames. Strict all-target
Clippy passed for Core, Audio, Catalog, std offers and std Host. The documented
public xtask command passed both Source checking and its production-kernel oracle.
The earlier wire evidence above remains historical; this validation covers the
nominal-unit ABI. Parent #5223 and all stated physical/perceptual boundaries remain.
