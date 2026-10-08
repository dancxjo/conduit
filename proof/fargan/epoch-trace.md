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

The optional authored `*_traced.conduit` variants preserve the original three
feedback domains and expose separate feature/conditioning, history and committed
PCM observations. The trace proposal is admitted from the same numerical pair
and original feature context as the conditioning proposal. Its model anchor is
supplied by the already selected Source startup value in each projection.
The original default Source plots are unchanged. A library-only Source probe
checked and expanded the traced conditioning graph (47 nodes, 53 cords) and
compound graph (800 nodes, 1180 cords); it executed no trained network.

`development_trace_sink.rs` stages a validated row, consumes it transactionally,
and records it only in `step_committed`. The isolated Source projection test
checks reference/prepared evaluation parity and paused observation, cancellation
before commit, duplicate refusal and zero allocation in step/commit. These are
component transaction-frame tests. The ordinary Plan/Play three-output transport
also passes with eight placements and six cords: all three exact typed/anchored
rows drain, and pressure/cancellation refuse whole trace completion. Its scheduler
uses fixed value storage after checked ingress transfer. Measured scheduler Step
and prepared expression invocations allocate nothing. This is transport proof,
not execution of the trained model. Complete traced utterance Source expansion also passes with all three feedback
cells and each diagnostic envelope below 16 KiB, within 1024 nodes/2048 cords.
Native trained all-row capture still requires its separate execution gate. The old genuinely
committed Travis-only carrier can support differential evidence; it must never
be relabelled as the complete greeting.

`fixed_storage_inventory.rs` measures a mechanical static envelope matching the
existing harness capacities on this x86_64 host: 19,909,952 bytes, including a
16,785,420-byte fixed value store and 1,441,840-byte fixed sign log. This excludes
numerical Back storage, admitted model bytes/views, Source preparation metadata
and all diagnostics. It instantiates no scheduler and executes no Source graph.
No FARGAN boot entry currently exists; no_std compilation is not boot execution.


`fixed_storage_ingress.rs` transfers the complete ordered live canonical ingress
into caller-owned fixed storage at preparation, preserving exact reference
identity, payloads and custody counts. Missing/reordered/foreign refs and capacity
failures clear only the new destination; occupied storage is preserved. Two
kernel-linked tests pass. The hosted driver retains the selected item and byte
limits when executing with this storage; it does not raise the pressure budget.
The debug constructor currently needs a 256 MiB preparation stack reservation
because constructing the 16 MiB array creates stack temporaries. The fixed store
itself is boxed during hosted preparation. Static target placement, prepared
numerical driver storage, fixed SignLog integration and boot execution remain
unproved; this startup workaround is not target SRAM admission.
