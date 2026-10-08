The optional development recorder observes actual published Source rows. It
confers no Native admission, parser commitment, model custody or recurrent ACK.
The Source final PCM transaction remains the feedback authority. No row is
recomputed to fill a gap. This is not a booted target recorder.

The exact checked profiles in `semantics/speech/fargan_epoch_trace.conduit`
have maximum canonical transport sizes of 5147 B (features20+conditioning320),
3107 B (next conditioning history128), and 12376 B (finalPCM160+state837).
A single PCM/state/features/conditioning/history record takes 17418 B and is
refused under the selected 16384 B envelope; no transport limit is widened.
Every separate record retains epoch and selected complete model/session anchor.

`development_trace_recorder.rs` prepares all row buffers before observation,
then validates canonical structure, exact anchor, contiguous epoch and finite
F32 leaves before copying. Invalid rows leave count/storage unchanged. It
refuses missing, duplicate, foreign, malformed, out-of-order and excess rows.
Structure plus these correlation checks do not replace full Source Native laws.
Call it only after actual Source publication, never as a substitute for that
publication. Runtime hooks and all-row capture remain unimplemented.

Each channel admits at most 256 rows. Three maximum-sized channels reserve
5,281,280 payload bytes, plus 768 Vec headers, retained schemas/validators/anchor
bytes and outer vectors. This development footprint is not fixed-target SRAM.
The isolated test measures zero allocation during two accepted observations;
preparation allocates. It does not prove a product Back's scheduler lifecycle.

Complete native 8 kHz timing supplies an exact positive multiple of 80 samples.
Feature 0 is retained separately for the five-call warm startup. For N native
epochs, the existing Source centered-condition chronology produces N+1 raw
rows using two explicit EOF epochs. Source alignment gives exactly twice the
native 8 kHz sample count. The extent helper accepts N <= 255 without changing voice
duration. Tests preserve 5040/63/64/10080 and cover 8240/103/104/16480 and a
legitimate slower 16240/203/204/32480 basis. These extent tests synthesize no PCM.
The current complete normal greeting is 1.03 s and does not satisfy the multi-second
acceptance. A genuine longer Source timing profile must be shared by both
realizations after full committed carrier admission; padding is not evidence.

Run `cargo +stable test -p conduit-plot --test fargan_trace_envelopes`.
