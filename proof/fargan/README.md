# Authored full-float FARGAN graph

The optional `conduit-speech/fargan-source` feature exports checked-source
inputs for the frame conditioner, complete forty-sample recurrent synthesis
subframe, and one composed 160-sample frame. Network order, reset-after GRU gates, pitch-history indexing, and
state proposals live in `semantics/speech/fargan_*.conduit`. Generic AI numerical
owners implement the fixed arithmetic through the ordinary kernel operation
factory. No upstream C code is linked into Play.

The subframe publishes de-emphasized float PCM and its entire next active state
in one result. A caller must accept that result before advancing recurrence.
The conditioner exposes the two-frame history and one-frame pitch delay
explicitly. `speech/fargan-frame` connects it to four sequential subframes in
one checked graph. A caller stages all six frame outputs before committing
next state/history/period. The ordinary prepared realization has 768 nodes and
1,211 Cords, with an explicit finite queue and stateless-instance profile. These Value entries do not establish reusable streaming, a feature
extractor, PCM16/device output, intelligibility, or real-time assurance. Those
claims remain with the speech work's own acceptance criteria.

## Private model differential and performance

Repository performance work enters through:

```sh
cargo xtask prove runtime-performance --fargan-fixture /absolute/private/fixture --output /absolute/new/result --repetitions 3
```

This extends the #5395 measurement entrance and reuses its timer control,
process CPU clock, allocator counters, source metadata, and raw JSONL format.
The command requires clean committed source and a new output directory; it
retains conformance, worker logs, source custody, and artifact hashes. It never
downloads a model or publishes parameters. The supplied local fixture includes
`resources/manifest.json`, `resources/f32.bin`, and
`full-frame-oracle.bin`. Hash and shape checks refuse a different bundle.

The pinned upstream is xiph/opus
`503d81b138d76621aae4b12786e90de48aa8db3a`. Its generated full-float model arrays
have SHA256 `68ba6e8731007f7184be862d7d1afe4cef096a7c9bcbfa115962340f725e004c`;
the exported immutable F32 bundle has SHA256
`d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47`.
The scalar development oracle is compiled independently with contraction and
SIMD disabled. Separate differential fixtures supply 96 subframes with exact oracle
conditioning, 24 conditioners, and 24 complete frames seeded from explicit warm
state. The full-frame trace SHA256 is
`9c676f6c76aec71281ea097fe0c477c5a3b77f5022e639a0d82dbb4a5437fcc3`.
Development-only exporter and independently built scalar oracle sources are
retained in `development-reference/`; model arrays remain externally supplied. Numerical guards describe this finite differential and the explicit
libm profile, rather than a global error theorem. Model redistribution terms
remain unresolved; parameter files are not checked in.

The worker reports Source checking, first-use model reading/resource admission,
and Plan preparation separately from owner preparation and Play. Process-first-use
measurements do not imply a cold operating-system file cache. Each useful output contains 160 PCM samples. Numeric
Back timing includes codecs; the Step remainder includes dispatch, transport,
mandatory Signs, Back commit, and timing-observer work. Expression Host Calls
are separately timed. The hosted proof worker explicitly requests a 32 MiB
thread stack; inline scheduler bytes and process allocator live/peak bytes
retain their separate meanings. Timers are not subtracted. Allocation counters cover
Play after all fixture buffers are prepared, through structural drain and
consumption of every Host Call completion, before result decoding. Repeated
Value invocations are not represented as a persistent stream.

The explicit hosted catalogue-cache feature retains only immutable checked
built-in Types and contracts, never tensors or execution state. First-use
catalogue preparation remains inside the cold schema measurement. Freestanding
and default profiles do not enable this std-only preparation cache. It is not
credited as a scheduler improvement.

## Recovery boundary

Selected generic numeric operators, authored subframe sources, and finite
oracle fixtures were recovered from archived revision
`f813938ba22cb23e36181f3e1b9ee2e3c3edb61c`, then adapted to current parameterized
Types and exact canonical port envelopes. The numerical contract revision is
`conduit.numeric/fixed-f32-libm@3`; old nominal Type identities and native laws
are preserved. Archived feature extraction, native audio, training, compact
precision, and streaming subsystems were not restored by this slice.

Local release contract tests cover the full graph, explicit pressure and host
cancellation, and retention of earlier native Type identities and laws. The
full-frame pinned differential carried its own history and signal state across
24 frames: maximum absolute PCM error was 8.781e-6, signal-state error 0.013958,
and conditioning-history error 5.866e-5. These are local hosted implementation
results for the explicit libm profile, not accepted-release or completed issue
status. Retained performance evidence is a separate obligation.
`proof/performance/verify-fargan.py` verifies source/model/Plan custody, complete
phase coverage, zero Play allocations, structural drain, and exact repeated
output/Sign/work parity. Its comparison mode reports repetition-cluster
bootstrap intervals for descriptive timing ratios.

## Actual-carrier endpoint profile (development)

`cargo +stable xtask check fargan-carrier-endpoint` checks the public mathematical
coefficients, actual consecutive carrier epochs, opaque Source endpoint admission,
and generic PCM framing without private model parameters. The finite profile
requires exactly 50 complete 80-sample epochs at 8 kHz with the same original,
unreduced cycle ratio. It preserves the original carrier, projection, Audio
basis, period and control receipts. Gaps, tails and unsupported controls refuse.
Its causal 16 kHz analysis frontend is an explicit spectral approximation.

The separately checked endpoint Source describes 51 raw synthesis calls:
features 1..49 followed by two explicit copies of feature 49. Warm initialization
uses 320 unheard zeros and five copies of actual feature 0. Ordinary Source
concatenation assembles 50 audible 160-sample packets; both unheard 80-sample endpoint
halves remain evidence outputs. No producer output is silently abandoned.

The development-only private numerical comparison is:

```sh
cargo +stable xtask check fargan-neural-endpoint
```

It requires absolute `CONDUIT_FARGAN_DEVELOPMENT_FIXTURE`,
`CONDUIT_FARGAN_ENDPOINT50_ORACLE`, `CONDUIT_FARGAN_ENDPOINT50_BUILD_RECEIPT`
and a fresh `CONDUIT_FARGAN_ENDPOINT50_OUTPUT` directory. The oracle is built
independently from pinned upstream scalar code; the command downloads nothing.
It checks all 51 raw frames, accumulated state/history/period, 8000 audible samples
and both unheard halves. Product recurrence starts from its own accepted Source
warm outputs; C state is comparison evidence only. Frozen maximum errors remain
PCM 0.002, state 0.05 and history 0.001.

These development checks establish local numerical correspondence and fixture
staging. They do not establish an installed atomic streaming controller,
perceptual quality, physical playback, target cadence or stable acceptance.
