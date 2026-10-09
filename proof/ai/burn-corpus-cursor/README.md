# Finite durable corpus cursor

Based on `d932da60c7f04bde62c797126e298e38aa9819d0`, extending the existing
Burn realization of #2140 training semantics. Existing Data dataset manifests,
shards, split occurrence identities, tensor content digests and AI TrainingBatch,
TrainingSession and TrainingState remain authoritative. No new portable model or
training ontology, corpus downloader or speech trainer is introduced.

The host prepares an immutable finite collection of actual semantic batches and
inline input/target tensors. It declares explicit batch count, total tensor bytes,
epoch and short-final-batch bounds. Every split occurrence appears exactly once
per epoch. Stable order preserves the declared batch order; the pinned seeded
profile sorts indices by SHA256(profile, little-endian seed, epoch, batch index),
with index as the tie break. Within each declared batch, existing Stable order
semantics remain intact. The provider-owned next operation accepts no caller
batch, tensors or cursor. Ordinary model/work TrainNext uses that same owner.

The exact corpus digest includes complete Data dataset/split semantic digests,
full canonical TrainingBatch frames and every input/target tensor semantic digest.
The complete TrainingSession digest binds preparation to the admitted context.
A committed step publishes model/optimizer progress and its prepared next cursor;
failed/cancelled work and evaluation preserve both. Attach admits a conservative
retained-owner plus transient-next-batch/cursor staging estimate alongside the
existing model working estimate. Top-level batch and tensor vector capacities
are bounded; validated nested carriers are cloned into fresh bounded storage.
The estimate includes metadata, order/keys, and cursor storage; it is not a hard
whole-process heap ceiling or a claim that tensor bytes cover preparation.
Successful unload releases the prepared corpus and cursor. Every declared Batch
axis must equal the batch example-membership count; context tensors without a
Batch axis do not acquire one implicitly.

Checkpoint descriptor format 2 carries the compact corpus identity, recipe,
epoch and next-batch cursor alongside immutable model/optimizer blobs. Resume
validates a freshly prepared exact corpus and cursor/step correlation before
loading candidate model/optimizer state. The permutation is regenerated, not
serialized as an unbounded list. Descriptor format 1 remains explicitly manual
batch or inference-only; None cursor fields are omitted. Readers reject mismatched
format/cursor presence. Model state schema remains the architecture's schema.

In-memory committed training progress and the durable latest pointer are distinct.
A failed later checkpoint leaves the live committed cursor advanced while retaining
the prior durable latest pointer. Resume from that pointer restores its earlier
exact next batch. Existing durability-uncertain outcomes remain explicit.

This addresses a generic checkpoint/resume gap in #5205. ConduitVoice, tutorial,
accelerator and whole-issue acceptance remain separate obligations. Runtime and
validation evidence is recorded only after the corresponding gates finish.
