# Independent canonical TLS guest

Source preparation only: no execution, timing, stack-fit, certificate or hostile
peer proof has been run. Uses embedded-tls 0.19.0 public **async** API unchanged,
Aes128GcmSha256, CertVerifier<NoClock,1024>, configured server-name verification,
CertificateVerify and MaxFragmentLength::Bits11. No NoVerify, blocking replay,
WouldBlock restart, TLS fork, scheduler, socket or device authority.

## Root proxy contract

The pure `conduitos-tls-core` exports `Operation`, `Failure`, `Refusal`, `wire`,
`state` and memory/buffer constants. Guest binary is `conduitos-tls-service`.
All wire functions encode/decode exact 64-byte CTLS version-1 little-endian
records; Envelope has nonzero instance and sequence. Sequence increases for
EVERY entry. Raw transfer window is 16 KiB, no address-valued DTO fields.

1. Initialize: `state::encode_initialize(seed32, host, cert)` packs the Root-approved
   independent cryptographic seed, hostname UTF-8 (1..64 bytes), and pinned DER
   (1..1024). Upload at window offset zero; Init carries host_bytes/cert_bytes.
   Ready creates one connection only; it does not authenticate or start IO.
   Root clears its seed/initialization copies. No reinitialization/reconnect.
2. Start {operation,kind,bytes}: strictly consecutive operation IDs beginning 1.
   Open/Flush/Close require bytes=0. Write uses plaintext window bytes in
   1..16384; Read requests a caller capacity in 1..16384. Started is a queued
   operation acknowledgement, not progress or successful authentication.
3. Poll {operation}: exactly one poll of the SAME retained pinned future. A
   Pending poll MUST return exactly one outstanding correlated IO grant:
   - NeedRead {operation,event,maximum<=16640}: Root reads TCP at most
     min(maximum,16384,remaining cumulative allowance). Provide carries this
     exact op/event and positive count; ciphertext is at transfer offset zero.
   - NeedWrite {operation,event,bytes<=4096}: Take chunks at exact contiguous
     offsets into the transfer window. Chunk's op/event/offset/count/total must
     match. Root writes every byte successfully before Ack for this op/event.
   - NeedFlush {operation,event}: Root flushes the actual TCP stream successfully
     before Ack. No guest descriptor grants permission to connect/reconnect.
   Provide/Ack return Ready; then Poll again. Do not Poll an unacknowledged grant.
   Event IDs are global, nonzero and monotonically consecutive across operations.
   Each event is acknowledged once; Take cannot replay offsets. No Pending
   without an IO grant is accepted. Root retains finite poll/wire-byte/deadline
   budgets around the complete operation, checking its actual clock each poll.
4. Done {operation,kind,bytes}: matching operation only. Write returns canonical
   partial write count (1..submitted length); Root's embedded_io::Write returns
   that count so its existing write_all performs the next operation. Read count
   is at most requested, including zero: no plaintext download on zero. Other
   operations report zero. For positive Read, Plaintext chunks have event=0 and
   exact contiguous offsets/counts/total. Download completely before Start next.
5. Failed {operation,error}: Authentication/Handshake/Write/Read/Close is final
   for this command and connection. No implicit retry/replay/recreate. Root must
   quarantine. Failed Close does not retract an earlier completed authenticated
   operation: Root still owns TCP retirement and the completed-operation receipt.

Every malformed/correlation/refusal/hardware/IO/deadline failure is terminal.
Root EOF or IO failure quarantines WITHOUT acknowledging or resuming the guest
future. Root preserves original pinned endpoint/provider/Boot checks, TCP polling,
TLS-to-WebSocket mapping, TCP retirement and close-refusal reporting. The existing
Network guest remains independently live above this TLS proxy. Guest replies are
not Host/Boot/semantic authority and do not establish DMA/driver isolation.

## Lifetime invariant

`Session.pending` is one Pin<Box<dyn Future>> OWNING Box<TlsConnection> while the
canonical method borrows that connection across await. It never borrows a field
of a movable Session. The async compiler owns those self-references inside the
pinned operation. Completion drops the future before returning connection
ownership to Session; no second operation can access it concurrently. Close
consumes it. Cancellation/drop destroys pending future first, then idle connection.

The only lifetime extension is the one-time private Resources loan. RX/TX/name/
DER are separately allocated and privately raw-owned. No Box owning those buffers
is moved/retagged while borrowed. Their addresses remain stable; raw owners never
read, reborrow, mutate or free them while TLS loans exist. References cannot
escape the private session/future. Session Drop ends all connection/future loans
before Resources erases and frees allocations. Root quarantine clears the ENTIRE
metadata/stack backing, including future captures, keys and freed heap blocks.
No TLS pointer, key schedule, verifier pointer or borrowed Root buffer crosses IPC.

## Finite profile and crypto implementation

Image400000..600000 (2 MiB RX/RO); exchange601000 (4 KiB); transfer603000 (16 KiB);
stack608000..708000 (1 MiB); metadata709000..909000 (2 MiB). First metadata4KiB is
bounded initialized RW/NX compiler data only. State is at +4KiB (assert<=4KiB),
allocator cursor+8KiB, bitmap+12KiB, heap after+20KiB. Buffers remain RX16640 and
TX4096, independently from transfer chunks. Metadata is an engineering ceiling,
not measured runtime fit; linked-image and stack admission remain required.

Existing guest CR0.TS deliberately rejects SIMD/FPU. Guest builder MUST pass
`--cfg aes_force_soft`, `--cfg polyval_force_soft`, and
`--cfg curve25519_dalek_backend="serial"`. tls-service's direct sha2 0.10.9
`force-soft` feature selects software SHA2 ONLY in this separate guest build.
No SIMD authority is enabled. Root crypto acceleration stays unchanged.

Dependencies: embedded-tls0.19.0 (default-features=false,rustpki,ed25519),
embedded-io0.7.1, embedded-io-async0.7.0, rand_chacha0.3.1, rand_core0.6.4
(default features disabled); guest adds sha2 software feature and existing
compiler-memory routines. Guest must remain a separate Cargo workspace/build
for its software crypto configuration; Root may separately consume tls-core.
