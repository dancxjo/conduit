# Authored finite acoustic PCM proof

Run the development check through the repository entrance:

```sh
cargo xtask check acoustic-flow
```

The deterministic hosted integration fixture composes the accepted periodic
oscillator and resonator Source with explicit signed-16 clipping, retained scan
state, a Source timing projection, and `audio/pack-pcm16`. Its four clock inputs
produce four mono signed-16 little-endian PCM blocks at 8 kHz. Each block contains
one sample and the existing 29-byte PCM header, with clock identity 1 and start
frames 0 through 3. The first sample is asserted as 24. This short digital fixture
establishes finite transport; it does not establish an audible tone or physical
speaker output.

The acoustic algorithms and the decision to emit or skip a sample live in
Source. Packing accepts a checked Native `AudioPcm16PackingInput`, validates the
existing PCM header constraints, checks integer narrowing, and serializes bytes.
It never clamps, synthesizes, resamples, or opens a device. Invalid rates, zero
clock identities, overflowing frame intervals, malformed values, and samples
outside the signed-16 range refuse. A `skip` input emits nothing. Flow closure
owns end-of-stream; the header flag means discontinuity.

The scan uses the production activation coordinator and prepared child kernels.
The child has 64 Gears, including an explicit bounded identity that preserves the
state envelope. The parent uses the normal kernel with exact initialized scan,
local expression, and packing profiles. The Rust fixture binds typed root ports
and exercises that Plan; it does not construct the synthesis graph. The scan
retains oscillator and resonator cursors between inputs without caller feedback.

Preparation admits the four child invocations, fixed evaluators, packing storage,
and transport queues. The packing input contract is at most 32 KiB, its sequence
contains 1 through 1024 integers, and its output is at most 2077 bytes. Queue
capacity is a separate finite reservation: this fixture uses the larger checked
DSP child envelope for the planner's shared queue setting. No kernel table limit
is increased. Allocation probes measure advancement after preparation; they do
not count the already admitted storage as zero memory.

The check also covers atomic Value pairing, exact profile refusal, child output
pressure and cancellation, retained cursor counts, and packing transaction retry,
commit, cancellation and exhaustion. Broader partition invariance and stream
lifecycle acceptance belong to #5683. Source, checked Plot, expanded Plot, Plan,
and Play remain distinct identities. The fixture is a development proof, not a
stable release receipt.

The authored graph and PCM projection are in
`targets/std/tests/fixtures/acoustic-value-frame.conduit` and
`targets/std/tests/fixtures/acoustic-pcm-projection.conduit`. The parent specimen
and its transparent initial cursor are composed in
`targets/std/tests/acoustic_pcm_parent.rs`; the ordinary scan syntax is retained
separately in `targets/std/tests/acoustic_scan_source.rs`.
