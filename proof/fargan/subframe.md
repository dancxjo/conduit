# Authored full-float subframe development proof

`semantics/speech/fargan_subframe.conduit` composes the checked signal and pitch/history source graphs. Its only externally committed result contains 40 finite de-emphasized floating PCM samples and the complete next active state: convolution history164, GRU160/128/128, pitch history256 and deemphasis1. This is 837 state scalars; the canonical result occupies9,962 bytes. Source owns network ordering, recurrent equations, pitch indices, gain normalization and the .85 deemphasis coefficient. Generic numerical owners receive ordinary exact resource/tensor inputs.

The ordinary test expands the authored source, checks and seals a Plan, lowers it, selects generic std numerical owners and the existing hosted ConduitOS expression owner, then runs the fixed scheduler. Loader resource handles are dropped before execution. Immutable admitted Arc custody stays with the selected numerical Backs. The synthetic fixture uses zero matrices and nonzero input/state/output bias, with an independent f64 expected PCM recurrence and state checks. It establishes ordinary host execution, not a booted target or reusable stream.

The ignored private differential requires an explicitly supplied local fixture directory through `CONDUIT_FARGAN_DEVELOPMENT_FIXTURE`. It validates the model blob, generated-array provenance, each selected array digest and exact shape, then compares96 source-authored subframe invocations against a pinned scalar full-float C oracle. It carries its own resulting state between calls while taking the exact oracle conditioning input. Preparation and execution times are reported separately. The owner uses libm activations; upstream scalar activation approximations differ, so bit parity is not expected. Empirical error must be read from the actual run, not inferred from activation error alone.

No weights or generated upstream model sources are included. An explicit redistribution grant for the acquired pretrained model remains unresolved. The public wrapper and build script consume caller-supplied local sources/model data, perform no downloads, and serve only as a development oracle; the product runtime does not link this C implementation.

```sh
proof/fargan/build_subframe_float_oracle.sh OPUS_SOURCE MODEL_DATA_DIRECTORY OUTPUT_EXECUTABLE
OUTPUT_EXECUTABLE > FIXTURE_DIRECTORY/subframe-float-oracle.bin
CONDUIT_FARGAN_DEVELOPMENT_FIXTURE=FIXTURE_DIRECTORY \
  CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo +stable test -p conduit-ai --test fargan_signal_graph \
  pinned_scalar_full_float_subframe_pcm_and_accumulated_state_differential -- --ignored --nocapture
```

The oracle pins xiph/opus revision503d81b138d76621aae4b12786e90de48aa8db3a and selected numerical source digests. Its output is96 fixed records of7,180 bytes: little-endian periodI32, conditioning80F32, prior active state837F32, PCM40F32, and next active state837F32. Gold SHA256 is `bf52f0431af52435e775818d7e7fe65ccf8b6ff7de38e57eab4b0db9935feccf`. The build deliberately omits `DISABLE_DEBUG_FLOAT`: scalar selection alone would still admit compact/hybrid matrices under that flag. Startup follows five first-feature conditioning calls and a320-sample silent continuation before the measured24 frames.

This fixture does not establish acoustic feature generation from a NativeSpeechPlan, phoneme conditioning, intelligibility, realtime timing, repeated Flow execution, compact-profile parity or public deployment.

The measured96-subframe full-float run passed with accumulated PCM maximum absolute error `0.0000044665067` and complete active-state maximum absolute error `0.008596808`. Total preparation was434.097667s and execution24.417735s; these timings expose per-invocation preparation overhead and establish no realtime claim. The scoped regression guards are PCM `<1e-5` and active-state `<1e-2` for this pinned fixture. They are empirical bounds for this run/profile, not a theorem for other signals or models. The separate conditioning graph must be measured independently; this subframe differential uses oracle conditioning.
