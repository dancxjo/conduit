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

Canonical Face transport has a separate prepared specialization:
`InitFace` (tag 10) selects `FaceScheduler` with an 8192-byte per-value envelope.
The legacy native scheduler remains 3072 bytes; text and Morse specializations
retain their own limits. All use the same kernel, fixed node/cord/value counts,
and 24,576-byte aggregate native value budget. The Face specialization reserves
finite remote Sign storage explicitly. This does not advertise a new native
Patchbay Back or change its retained production Source by itself.

`AdmitRemote` (tag 11) carries an exact admitted endpoint, cord, sequence and
transfer length. Only the Face specialization accepts it. Its distinct
`RemoteAdmission` response reports Accepted or Full with the same sequence;
Full is live pressure and consumes neither a value nor a sequence. Legacy,
text and Morse owners refuse the command. Root's separate Face broker retains
only exact admitted Fore custody receipts and publishes byte loans after guest
acceptance. Unknown routes, stale sequences and oversized inputs refuse before
entry; uncertain transport results quarantine the broker without replay.

Initialized Document output has its own one-shot Root loan of exactly eight
zero bytes. It grants no empty edit and no second initial publication. Subsequent
Editor/Document values require the existing exact Root-completed value loans.
Hosted deterministic ledger specifications cover that boundary independently of
any claim about executing the protected service.

The current linked release image reserves 501,128 bytes in Runtime::execute,
195,160 in Face admission construction, and 190,936 in the nested 8192-byte
scheduler constructor (local reservations, excluding saved registers and further
callees). These observations do not establish complete peak stack fit or timing
within the unchanged 1 MiB native stack. Unoptimized hosted Runtime tests need
an explicitly larger test thread stack; that is not a native stack change or a
substitute for native enactment. Native canonical Patchbay integration and
platform acceptance remain incomplete.

The Face profile reserves 1024 remote lifecycle Sign events in the existing
`FixedSignLog<1024>` backing, not ten endpoint records. Mandatory retained history
still eventually refuses at finite Sign pressure; there is no hidden eviction or
replenishment. A focused regression dispatches thirteen inputs on one endpoint,
so the number of cords cannot accidentally become a ten-interaction session cap.
