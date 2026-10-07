# Source conditioner and signal feedback development proof

`speech/flow-fargan-compound-cycle` composes the reusable conditioner and
four-subframe signal cycle. Separate, explicitly admitted seed domains carry
conditioning history and signal state. The final canonical PCM16 epoch is the
only return acknowledgment for both domains. Layer ordering and feedback policy
remain authored Source; the host registers generic retained-schema owners.

The standalone conditioner also compares the full selected artifact, descriptor,
session anchor and precision before accepting a PCM acknowledgment. Exact epoch
and checked increment guards reject a mismatched epoch or overflow. Source
matcher tests exercise reference/prepared parity and reject foreign identities.
Matching an anchor does not admit a model or linguistic session.

The private ignored test
`retained_model_executes_compound_conditioning_signal_cycles_with_final_pcm_ack`
uses `CONDUIT_FARGAN_MODEL_FIXTURE` and the locally exported pinned full-float
bundle. Both synthetic zero-feature epochs use the same 33 immutable tensor
views from one admitted model blob. One ordinary Plan executes both frames;
there is no per-frame replanning or Value owner reset. The scheduler drained
both feedback return debts without a surplus output and emitted two 160-sample
canonical PCM16 aggregates with epochs 7 and 8 and finite next state.

The selected-anchor version measured 832 placements and 1290 cords. Planning,
owner preparation and execution took 120.10, 201.91 and 11.96 seconds respectively
in one hosted debug run. These timings establish no realtime claim. This test
uses synthetic features and independently admitted synthetic seeds, rather than
Source warm startup or a committed native linguistic plan. It establishes no
native utterance waveform, quality, booted target execution, compound inference
interface admission or model redistribution permission.

The additional private conditioning and compound `ModelSignature` tests prove
logical descriptor custody: the full artifact plus signature descriptors are
distinct while sharing the same full content, exact read grant and immutable
blob allocation. These signatures describe logical F32/U16/I16 tensors. A
complete admitted mapping to the retained native structured Source carriers,
state ordering and compute lifecycle remains separate required work; descriptor
custody alone does not establish that mapping.
