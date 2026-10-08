# FARGAN model-compute admission boundary

Issue#4898's current model-identity section explicitly requires reuse of
`ModelComputeOffer` and model/working/device-memory bounds. Its execution
acceptance additionally requires ordinary Plan/Play, finite streams, pressure
without recurrent/progress commit, distinct cancellation/completion/provider
loss/refusal, and a bounded selected runtime. The hosted static common greeting
at5a72905 proves a trained Source/Kernel computation and finite service envelope;
it does not establish general ModelComputeLifecycle acceptance.

Existing owners are `semantics/ai/src/model_compute.rs` and the corresponding
Source vocabulary in `semantics/ai/types.conduit`. `ModelComputeOffer::admits`
checks operation, format, element, rank, compute class/lanes/service guarantee,
solver/determinism/checkpoint profiles, model/working/device/input/output bytes
and batch size. Offers have explicit queue bytes/items, one in-flight operation,
cache policy and cancellation support. `ModelComputeSession` retains runtime
identity, loaded digest/bytes, bounded queued counters and lifecycle transitions:
discovered→loading→warming→ready→active→ready; provider loss clears loaded/queue
state; explicit unloading→shutdown releases the logical model. These owners
must be reused rather than replaced by a FARGAN service-counter enum.

A concrete hosted capability proof must add an opaque adapter around the actual
admitted model resource and selected immutable Plan/runtime owners:

1. Retain the exact `AdmittedModelResource`, descriptor/signature/content digest,
   actual packed/shared resource bytes, primitive/adapter artifact identities,
   numerical precision/determinism profile and observed host identity. Select an
   existing ModelComputeOffer using a requirement derived from those owners.
2. Inventory all admitted preparation, immutable resource, scheduler, recurrent,
   Host Call pending/input/output, scratch and queue allocations. The16MiB
   ValueStorage ceiling alone is not a whole-runtime working-memory bound; peak
   RSS alone is not a reusable admission contract. Current original full-carrier
   preparation is separately capped at256MiB. Do not invent a small working
   bound from model weight bytes or infer a realtime service guarantee.
3. Bind `begin_load` to the actual exact resource digest/byte admission,
   `begin_warming` to the actual five warm-up repetitions, and `ready` to their
   terminal receipt. `begin(requirement)` has no model-identity parameter; the
   opaque adapter must keep and recheck the same retained resource/Plan rather
   than assuming that the lifecycle state proves model correspondence.
4. Admit a finite input queue/cadence/byte basis before enqueue, transition to
   active inference only for that selected requirement, and retain the actual
   Source/Kernel services, full outputs and quantum boundary receipts. Call
   finish only on actual scheduler drain with the exact expected outputs.
5. Exercise the real scheduler with output pressure at staged cursor/state
   boundaries, then resume; compare complete Native outputs with uninterrupted
   execution. Confirm pressure cannot commit recurrent/progress state. Exercise
   cancellation and provider/model loss against actual pending work, distinguish
   them from completion, and verify queue/resource reclamation and unload.
6. Add negative gates for foreign/truncated resources, changed descriptor or
   Plan, insufficient model/working/queue capacity, unsupported format/precision/
   lanes/service, malformed/stale frames and invalid lifecycle order. Preserve
   all original inputs and refusal evidence. Repeat on the accepted ConduitOS
   runtime before target claims; a hosted component cannot substitute for it.

The existing state machine has a `failed` vocabulary variant but no public
failure-transition method. That gap must be handled through the existing owner
contract or a separately reviewed generic extension if a concrete runtime
failure needs it; do not report a failed operation as `finish`/completion. Its
queue `begin` consumes caller-supplied queued bytes, so the adapter must retain
exact queued-item ownership/accounting rather than pass guessed counters.

Listening, physical playback, target resource/stack/image measurements and rich
prosody are separate acceptance evidence. No current finite-service or
nearest-period component claims those proofs.
