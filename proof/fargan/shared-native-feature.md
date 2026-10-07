# Shared native feature replay

The private slow `Hello, Travis.` handoff is pinned by JSON SHA256
`f8d5a958ec0751eccedbbcdfb9dd2d3445b53ee45512399b1d081532eba1c467`.
The development replay retains the whole learned graph receipt and native intent,
voice, inventory, boundaries, event spans and ten pitch admissions. It rechecks
native admission, the compiled formant Source identity, all 200 paired 8/16 kHz
pitch receipts, and all 16240 mono 8 kHz WAV samples. These are 2.03 seconds of
actual realization, with no padding.

Set `CONDUIT_FARGAN_SHARED_HANDOFF` to the private JSON path, adjacent to its
same-basename WAV. Set `CONDUIT_FARGAN_FEATURE_EVIDENCE` to an optional output
JSON path, then run:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo +stable test -p conduit-ai --features kernel-operation-owners \
  --test fargan_feature_policy retained_shared_native -- --ignored --nocapture
```

The Source approximation explicitly resamples the first four actual 80-sample
8 kHz epochs to 160 samples at 16 kHz, applies causal preemphasis, and updates its
640-sample analysis history. The period comes from event 0 at local frame 240
(8 kHz), projected through the same admitted pitch trajectory at 16 kHz and the
Source clamp/round policy. One ordinary expanded Source feature graph then
computes the 20 features. This named approximation retains the 4 kHz bandwidth
of its formant input; it does not establish upstream acoustic feature parity.

The actual expanded feature graph has 68 nodes and 75 cords. One local debug
replay measured 4.135 seconds of owner preparation and 391.75 milliseconds of
execution, separately from native admission, Source checking and planning.
These measurements establish no real-time throughput. Four preprocessing epochs
and one executed feature frame do not establish a whole-utterance neural stream.

The emitted evidence explicitly records `joint_committed_session=false` and
`neural_waveform=false`. Rich learned graph projection/native re-admission does
not substitute for the parser's stable/committed session receipt. The immutable
handoff and manual playback ACK do not establish joint linguistic stabilization.
No model weights are needed or redistributed by this replay.

The ignored epoch-flow test
`retained_native_feature_executes_authored_conditioning_with_shared_tensor_custody`
consumes the exact pinned feature execution receipt and executes
`speech/flow-fargan-conditioning-core` through an ordinary 24-node, 23-cord
Plan. Seven conditioning tensors are admitted slices of the retained model blob.
It produces finite conditioning320 and next-history128 values. The recorded run
used 52.32 ms of owner preparation and 23.92 ms of execution, separately; these
are hosted debug proof measurements. It uses an explicit zero-history seed.

The local `native-conditioned.json` evidence retains the complete preceding
feature/native receipt, authored Source, reviewed tensor layout, model identities
and numerical outputs. It declares `joint_committed_session=false`,
`conditioner_model_signature_admitted=false`, `source_warm_initialization=false`
and `neural_waveform=false`. A committed linguistic basis, compound model
interface admission, Source warm initialization and PCM-coupled history remain
required before the complete same-basis acceptance claim.
