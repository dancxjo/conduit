# Retained row scalar oracle (development only)

`build_retained_epoch_float_oracle.sh OPUS_SOURCE MODEL_DATA_DIRECTORY OUTPUT`
checks the same pinned upstream numerical files and private model sources as the
subframe oracle. It does not download or redistribute either. The selected
revision is `503d81b138d76621aae4b12786e90de48aa8db3a`; omitting
`DISABLE_DEBUG_FLOAT` selects the full floating matrices. The C wrapper is a
local numerical oracle, never a product Back or linked runtime driver.

Input is little-endian: the actual first feature20 F32 row used for Source warm
initialization, then complete records of U64 epoch, I32 conditioned period, and
20 F32 features. Warm initialization repeats that first row five times with an
explicit zero continuation seed. Each observed record executes four upstream
subframes and retains conditioning, history, PCM and state. Partial trailing
records and periods outside the upstream embedding range are refused.

The optional ignored Native63 Source test uses `CONDUIT_FARGAN_NATIVE_TRACE`
to retain three exact canonical output streams only after scheduler commit.
`export_retained_committed_pcm_without_model_reexecution` mechanically decodes
those retained PCM records. `compare_retained_epochs.py CAPTURE ORACLE_OUTPUT`
then measures 320 conditioning, 128 history, 837 recurrent/pitch/deemphasis
values and 160 PCM samples per observed epoch. State component ordering is
oracle metadata only; product layer ordering remains authored Source.

PCM comparison explicitly applies the Source normalized clipping, F32 gain and
nearest-ties-away conversion to the upstream float result. It also reports the
upstream PCM quantizer separately. Source libm activations and upstream scalar
activation approximations are different numerical laws; this is empirical
same-input comparison, not a bit-parity or universal tolerance claim.

The committed Travis-only capture measured 64 rows, zero allocations in
scheduler steps and prepared expression calls, and a drained 969-node plan.
Conditioning maximum absolute error was 0.000405632; history 0.0000587106;
accumulated recurrent state 0.423375; PCM maximum 844 signed16 units and RMS
131.991. These results do not establish the cause of perceived audio quality.
The aligned actual PCM was byte-identical to the earlier Travis-only export.
This evidence does not claim a complete greeting, booted ConduitOS, full-loop
no-heap, real-time performance, or listening acceptance.
