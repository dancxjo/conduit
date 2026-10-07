# Source warm-start development proof

`speech/fargan-warm-startup` performs five conditioning calls with the supplied
first feature and period, starting with zero conditioning history. It then runs
four ordered subframes with explicitly zero continuation PCM, replaces the pitch
history with zeros after each subframe, and explicitly seeds deemphasis to zero.
Layer ordering, history replacement, and startup policy reside in Source. The
ordinary Value Plan executes once; it does not reset or rearm runtime owners.

The private development test is
`source_warm_startup_executes_five_condition_updates_and_four_zero_continuations`
in `semantics/ai/tests/fargan_epoch_flow/runtime.rs`. Set
`CONDUIT_FARGAN_MODEL_FIXTURE` to the locally exported fixture directory and
`CONDUIT_FARGAN_FEATURE_RECEIPT` to the retained native feature JSON, then run that
ignored test. Model weights remain outside the repository.

For the oracle inputs, encode the receipt's twenty `feature20` values as little
endian F32 and repeat that frame five times. Run the pinned full-float conditioning
oracle built by `build_conditioning_oracle.sh` and the full-float subframe oracle
built by `build_subframe_float_oracle.sh` on that input. The first subframe trace's
prior 837-scalar state is the completed warm-start state. Both scripts require
pinned local upstream sources and generated model C; they do not download or
redistribute weights. Full-float builds omit `DISABLE_DEBUG_FLOAT`.

The retained native feature receipt SHA256 is
`b8a72c0c8eedb09fbd56255180e37f1b96c65ff9b3b438672599ff5c9a2650d8`.
The five-frame conditioning trace SHA256 is
`8c151416cc237bbb9f911cc76065e42be0e781248440b569b2550203e95c84c2`.
The warm subframe trace SHA256 is
`a81f44cfe73d1a2f790199b229ae01dd55defa7f5b99ed390bf8c271812ae202`.

The ordinary Source Plan has 806 nodes and 1284 cords. One hosted debug run took
13.90 seconds for planning, 48.80 seconds for owner preparation and 2.99 seconds
for execution. All 33 tensor views share one retained immutable model blob.
Maximum absolute warm-state error against the pinned scalar full-float C oracle
was 0.00078777596; the scoped empirical bound is 0.001. Pitch history and
deemphasis were exactly zero. The five-call conditioner alone measured maximum
condition/history errors 0.00022575259/0.000057935715.

This fixture uses a retained native feature point at event 0, native frame 240.
Repeating that supplied point proves the named startup policy, rather than a
whole utterance's first-frame startup. It does not admit a compound model
signature, a jointly committed parser session, a native neural waveform, real
time, booted execution, or listening quality. Source uses its selected numerical
activation owners; these empirical differences do not establish bit parity with
upstream approximations.
