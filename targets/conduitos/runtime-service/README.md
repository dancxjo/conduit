# Native workset runtime service

This independently linked x86_64 guest runs the canonical fixed scheduler in
`conduitos-runtime-core`. Each Root command enters the independently owned
Runtime service profile, executes a single command and returns through the
existing service gate. Root retains the authoritative reviewed Host effect
bindings, Body/Play lifecycle and canonical application Host Call adapters.

The Runtime profile has an RX/RO image, a 4 KiB exchange, a 16 KiB transfer
window, a 1 MiB stack and 1 MiB private metadata. The retained scheduler begins
at metadata + 4096; a compile-time size assertion limits its admitted storage.
This assertion does not establish peak stack fit or execution timing. The first
metadata page is reserved for linked private data. No allocator is required.

The Root broker validates node/call/envelopes, exact pending completion identity,
at most one newly issued request per node between Root step commands, strictly
increasing request IDs, and Root-issued value/byte loans along reviewed routes.
Its finite loan ledger covers queued values and one undispatched input per Cord;
it does not select runnable work or execute a second scheduler. Initial empty
Application and Edit outputs have explicit one-shot empty grants.

Faulted/malformed transports are quarantined and released without retry. Root
fences pending actions, held inputs, views and output receipts after failure.
This source migration covers NativeWorksetPlay only: Tour/ordinary runtimes,
Root applications, Face/Mask services, drivers and networking have further work.
No runtime, physical, timing, DMA, or release acceptance is claimed by this code.

The first linked guest used a 392,280-byte Runtime dispatch frame and a
141,176-byte nested scheduler construction frame. The closed 1 MiB stack
accounts for those observed frames; complete peak stack depth and runtime
execution remain unmeasured. No emulator or physical proof is claimed.

Ordinary text admission is additive: `wire::Request::InitText { bytes }` (tag7)
consumes `wire::text_admission::encode(&text::Admission, transfer)`. This selects
an exclusive `Owner::Text` instead of `Owner::Native`; initialization cannot
replace a live owner. Its original scheduler type/capacities are retained:
3 nodes, 2 cords, 6 value slots/768 bytes, 64 Signs, 9 bindings, 3 pending slots.
`Stats` (tag8; response0x8008) returns canonical decisions/signs/pending counters.
Existing tags and Native Init bytes are unchanged. Text host input/output is
bounded to256 bytes. The Root ordinary adapter must independently validate its
checked literal/roles, exact node bindings, per-node RequestId1(Upper)/2(Face),
input loan provenance and finite drains before performing effects.

The same ELF can be admitted in the independent OrdinaryRuntime profile while
NativeWorkset's Runtime remains live. Its State is an enum, so storage is the
larger existing specialization plus its discriminant, not two live schedulers.
The retained metadata compile assertion remains `size_of::<Retained>() <=
METADATA_BYTES -4096` (1MiB metadata with first4KiB reserved). Source assertions
are bounds; linked stack/image measurements and runtime proof remain separate.

`runtime-core/text/backs.rs` is the canonical ordinary Literal/Upper/Face and
shared transform/sink implementation moved from Root's text_kernel_backs.
Root must reexport these definitions; its Timer/TickFace wrappers continue to
call the shared helpers. The core text bound256 and ports16 must be checked
against `conduit_text::MAX_TEXT_BYTES` and lowering's canonical port constant at
Root's preparation boundary. No new semantic reducer is introduced.

Tour fan-out/Morse uses additive `InitMorse { bytes }` (tag9) and
`wire::morse_admission::{encode,decode}`. Its five role ordinals are Literal,
Upper, TextFace, Morse, Indicator; configured literal is at most32 bytes.
The guest retains the original FixedScheduler specialization:5nodes/4cords,
16ports,4queues,80routes/4targets,25bindings,4pending,12value slots with3604-byte
storage capacity and3604-byte admitted budget,96Signs. Per-operation transfer
is at most645 bytes. Request IDs remain Upper1/TextFace2/Morse5/Indicator6,
HostCallId0. The shared Back definitions also serve Root's still-local comparison
scheduler; comparison admission/execution is outside this migration.

Native TourMorseKernel constructs only checked numeric admission and delegates
to the same Root Host Call broker using the independent OrdinaryRuntime lease.
This is sequential with ordinary text stages; NativeWorkset's Runtime lease
remains separate. Root retains protected uppercase/Morse/serial effect providers,
exact binding checks, initial literal and completed-value loans, and receipt
acceptance. Stats remains the existing additive counter query, with mode-specific
96Sign/4pending ceilings. ProtectedTimer is unchanged: that production path
already owns its canonical timer scheduler in its private TextDomain.


## Canonical interaction profile

`InitFace` (tag 10) selects `FaceScheduler` with an 8,192-byte per-value envelope.
The legacy native scheduler remains 3,072 bytes; text and Morse specializations
retain their own limits. Both native profiles use the same execution kernel with fixed node/Cord/value
counts and the 24,576-byte aggregate native value budget. Native workset preparation
selects this profile for the exact admitted Patchbay interaction Fore; this does
not widen every application value or make a second scheduler.

`AdmitRemote` (tag 11) names the admitted endpoint, Cord, sequence and transfer
length. Only the Face profile accepts it. `RemoteAdmission` reports Accepted or
Full with that same sequence. Full is pressure and consumes neither a loan nor a
sequence. Root's separate broker checks route, sequence and byte bounds before
entry and publishes custody loans only after exact acceptance. Uncertain guest
results quarantine that broker without replay. The complete Face/publication
uses the separate retained-application protocol; the 8,192-byte envelope does not
claim arbitrary full Face admission.

Initialized Document output has a one-shot Root loan of exactly eight zero bytes
(revision-zero empty text). It grants neither an empty edit nor a second initial
publication. Successor Editor/Document values need exact Root-completed loans.
The Face profile explicitly reserves 1,024 remote lifecycle Sign events in the
existing `FixedSignLog<1024>` backing, not one history slot per Cord. History is
finite, with no hidden eviction or replenishment; it is not a promise of 1,024
interactions.

See the [runtime profile delivery](../../../docs/audits/native-runtime-profiles-delivery-20261022.md),
[Root custody checks](../../../docs/audits/patchbay-root-custody-delivery-20261022.md)
and [installed caller cutover](../../../docs/audits/patchbay-installed-cutover-20261022.md).
Those records distinguish hosted checks, compilation and actual protected-service
execution. Native peak stack/private-heap fit and current-source QEMU enactment
remain unproven by the local compilation/CPU evidence; a larger hosted fixture
stack is not a native stack change.
