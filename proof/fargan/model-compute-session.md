# Hosted trained FARGAN lifecycle component

This test-scoped opaque adapter reuses the existing `ModelComputeOffer` and
`ModelComputeSession` around the actual trained Source FARGAN compound model,
sealed Plan, prepared owners and ordinary scheduler. It does not replace the
model with a mock, direct-C inference, or a lifecycle label-only simulation.
The original static full200 proof at `5a72905` remains separate and unchanged.

The session retains the actual compound model descriptor, complete canonical
signature, content/read-authority owner, executable and adapter identities,
original Source and sealed Plan, Source service admission, original warm
proposal, complete warm products and Source-generated feedback seeds. A queued
batch retains its complete original canonical frames in an immutable shared
owner. Execution receives an exact copy; only one original batch is retained
inside the session, so receipt history cannot grow without bound. Native port
names are fixed, all original frames receive recursive Source admission, and
seed frames must equal the actual Source-produced warm feedback. Foreign Plan,
malformed frame, excessive batch, duplicate queue, wrong output count, invalid
lifecycle order and post-stop execution refuse.

The named `hosted-value-storage-component-only@1` offer admits at most sixteen
frames and 256KiB of original input, one queued batch, one active operation,
256KiB output and the existing 16MiB ValueStorage component. It declares hosted
GeneralCpu/shared service and zero device storage. The model byte bound comes
from the actual admitted full model. These are component bounds, **not a full
working-memory bound**: AST/evaluator scratch, scheduler/sign arrays, Host
owners, Native/preparation receipts, allocator overhead and stack remain
uninventoried. `require_full_working_admission` explicitly refuses. A full200
batch needs a separately reviewed finite input offer; this small gate does not
silently broaden its sixteen-frame profile.

Load adopts the already admitted immutable resource. Warm executes the actual
Source warm graph. That graph produces three typed products and settles Idle
with four retained cells; it does not drain. The separately named warm
retirement accepts only complete products plus no pending Host work, explicitly
cancels that warm scheduler, verifies unchanged Source cursors and decisions,
and verifies zero remaining scheduler cells. Readiness retains this receipt and
the complete warm Source/Plan, rather than claiming a drained warm graph.
Inference finish still requires actual Drained with exactly the requested
number of results. The Source-backed finite service profile remains unchanged.

The actual scheduler probes distinguish:

- Normal: two trained compound feedback epochs drain, with storage and pending
  Host work empty, before `finish` returns the general session to Ready.
- Cancellation: cancel at an actual pending call, verify the next scheduler
  state is Cancelled, and prove no additional Source cursor/decision/result
  advancement at that frontier. The session refuses subsequent inference.
- Pressure: fill the unchanged 1024-slot store and observe actual output-store
  refusal before kernel Host completion. No subsequent cursor/decision/result
  advancement occurs. The session remains stopped Active; it is not finished.
  The existing `completed_host_calls` diagnostic counts the owner invocation
  whose output was computed; the explicit stop evidence distinguishes that
  from kernel Host completion.
- Provider loss: remove the actual selected prepared Host owner at a pending
  request, cancel the scheduler without another commit, transition the general
  owner to Lost, clear its loaded identity, and release its compute-owner
  reference.
- Unload: only Ready with no queued batch can unload and shut down. The compute
  reference is released; original model/descriptor/warm evidence remains in
  audit custody. This is not a claim that preserved audit weight bytes were
  physically freed.

Pressure currently stops and disposes the runner; resumable pressure equivalence
remains open. The hosted component also does not prove whole-working admission,
ConduitOS target execution, realtime scheduling, physical output, intelligibility
or the final continuous-word common-carrier neural projection. The two small
model epochs are explicitly typed fixtures, not a new greeting waveform.

User-facing evidence is under `outputs/fargan-model-compute-session/` in the
#4907 project. The original failed warm-Idle probe and the initial/extended
passing receipts are preserved separately; final proof records exact frozen
Source, Rust, model, SDK and executable hashes.
