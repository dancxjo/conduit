# Private signal model custody

`signal-layout-f32.json` records the 26 reviewed resource slots used by the
Source signal epoch. It contains shapes, offsets and digests, and no model
parameters. The pinned full float blob is 3,272,868 bytes; the selected signal
parameter slices total 2,989,684 bytes.

The ignored `pinned_model_retains_full_artifact_signature_and_one_shared_blob`
test loads an already acquired development fixture. It retains the complete
`ModelArtifact`, logical signal-epoch `ModelSignature`, read grants and blob in
`AdmittedModelResource`. Every admitted tensor borrows a bounded slice of the
same immutable `Arc<[u8]>`; parameter arrays are not copied. The inline
`TensorValue` descriptor is 280 bytes on this host, or 7,280 bytes for 26 slots;
Arc headers, dynamic descriptor metadata and prepared runtime storage are
additional costs.

The artifact semantic content identity, artifact/signature descriptor identity,
and raw blob SHA-256 are distinct. Session basis material includes the complete
unbound Source template, supplied caller receipts, layout/precision, descriptors
and grants before binding the selected frame anchor. The bound Plan is separate
execution evidence. This development loader alone does not admit a native
utterance, linguistic commitment, Source startup state or redistribution.

Run with an existing private fixture:

```sh
CONDUIT_FARGAN_MODEL_FIXTURE=/absolute/path/to/work/fargan \
  cargo +stable test -p conduit-ai --features kernel-operation-owners \
  --test fargan_epoch_flow pinned_model_retains -- --ignored --nocapture
```

The logical signature is scoped to conditioned signal epochs: a 320-element
conditioning vector, two periods and 837 scalar state elements produce 160 PCM16
samples, next state and next period. The conditioning producer, warm initialization
and native utterance session must be admitted separately.

The ignored `pinned_source_signal_cycle_retains_model_and_measures_free_running_error`
test executes the ordinary authored closing signal cycle with these retained
resources. The pinned scalar float oracle supplies 24 conditioning epochs and
the initial warmed state; subsequent state is entirely Source feedback. It
commits 24 canonical PCM16/state aggregates and drains after the final returned
state, without per-epoch replanning or state replacement from the oracle.

The development run measured maximum PCM16 error 1, RMS PCM16 error
0.1290994449, and maximum absolute recurrent-state error 0.0049425066 across
3,840 samples and 24 next-state observations. The regression limits are
respectively 1, 0.15 and 0.006, scoped to this exact pinned trace. This is not
bit parity or a general acoustic-quality bound. Owner preparation took
172.55 seconds and execution 122.34 seconds in the debug proof build; it does
not establish real-time performance. The graph has 781 placements and 1,230
cords. Source conditioning, Source warm initialization, native-utterance PCM and
booted target execution remain separate proof requirements.

`conditioning-layout-f32.json` additionally reviews the seven conditioning tensor
slots. `conditioning_resources()` admits them as slices of the same blob and
checks their exact Source port Types. It also requires the upstream embedding
bias to be all zero before selecting a pure lookup operation. These tensor views
do not broaden the logical signal-only model signature or claim an executed
conditioning session. The private custody replay checks all 33 slices share one
storage allocation.
