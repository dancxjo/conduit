# Bounded binary WebSocket service core

This preserves the native `embedded-websocket 0.9.5` calls and the 1,024-byte
upgrade / 32,768-byte frame / 32,754-byte binary-message ceilings. It does not
open a socket, authenticate TLS, approve an endpoint, operate a driver or mint
semantic delivery/Show evidence. The independently linked guest is preparation;
no execution, timing or fragmentation proof has been run for this extraction.

## Root facade contract

Keep `BinaryWebSocketIo` as the enclosing interface. Install one fresh Network
profile for one authenticated connection. Root retains the TLS/TCP stream,
provider/Boot/endpoint/certificate checks, deadlines and polling bounds.

All descriptors are canonical 48-byte little-endian `CNET`, version 1, with
nonzero boot-local instance and strictly increasing sequence. The 16 KiB transfer
window carries **raw bytes**. MAX_BLOB is 32 KiB, not a JSON byte-array bound.
Input transactions are Begin(kind,total), contiguous Write(offset,count) with
window offset zero, then Commit. Read output blobs at contiguous offsets; every
Read descriptor's count/total/offset must match Root's current output grant.
There are no pointer-valued descriptors and no implicit restart.

- Connect: `state::encode_initialize(seed, host, path)` encodes the Root-approved
  descriptor and independent cryptographic 32-byte WebSocket seed. Commit returns
  an Upgrade blob. Root reads it, writes all bytes and flushes the authenticated
  stream before reading its response. Seeded ChaCha20 and RNG erasure match the
  existing native carrier.
- Upgrade: Root reads initially at most 1,024 bytes. Each Handshake input Commit
  returns Accepted(trailing) when accepted or NeedInput(maximum remaining handshake
  capacity, consumed=0, written=0).
  Root reads at most that allowance. Post-header bytes remain in the guest frame
  receive buffer, exactly as in the native adapter. Full incomplete handshake
  returns HandshakeAuthentication; Root EOF/error returns HandshakeRead.
- Send: precheck empty/oversize payload as RequestTooLarge, then SendBinary input
  returns a WireFrame blob. Root copies, writes all and flushes; IO failure is
  FrameWrite and terminally releases this guest connection.
- Receive: precheck caller output length in 1..=32,754; StartReceive(maximum) runs
  one canonical codec read. Continue(consumed,written) means StepReceive without reading the stream.
  Each codec step reports consumed wire bytes and produced message bytes. Root
  checks those against its retained-wire and remaining-output ledgers and requires
  positive progress for parser Continue. Receive-input Commit Continue(0,0) is
  only a transport acknowledgement, not parser progress. NeedInput(maximum) means Root reads at most min(maximum,16 KiB), uploads Receive
  input, then StepReceive. Never discard retained bytes between these calls.
  Blob descriptors include the exact remaining buffered wire-byte count for the
  next operation. BinaryMessage returns the complete accumulated payload, including legal empty
  messages (zero-length blob: no Read requests). Copy it only into this caller's
  admitted output. Non-binary/control frames retain UnexpectedFrame behavior.
- Close: Close returns canonical normal-close WireFrame bytes; Root writes and
  flushes, reporting Close on failure, then performs its existing TLS/TCP close.

Root retains a finite deadline/poll/step bound around the complete operation;
Continue is a parser step, not permission for an unbounded Root loop. On any
transport/descriptor/correlation failure, quarantine rather than reuse stale
bytes. Root stream EOF/failure maps to the existing HandshakeRead/FrameRead errors
and terminally releases the connection. Local empty/oversized send arguments or
invalid caller receive-buffer bounds return their existing errors without a
service entry or connection mutation.

A correlated guest `Refused(WebSocketError)` is accepted only when that exact
codec operation can produce that error. It fences ordinary send/receive work,
but Root may perform the existing explicit Close on that same live instance if
the canonical codec state permits it. A codec error can consume bytes before
refusing, so the previous retained-byte ledger is then only an upper bound for
Close; ordinary receiving does not resume with uncertain progress. Close still
requires a bounded guest-produced frame and Root-owned write/flush, followed by
normal lease release. This is not automatic retry or reconnection.

Malformed/correlation/backend faults, guest transport Bounds/Order/Capacity
refusals, unrelated fabricated codec errors, and every Root IO failure remain
terminal and quarantine immediately. Guest-reported IO errors are never accepted
as codec refusals: actual stream IO and its failures remain Root-owned.

## Required memory profile

Image 0x400000..0x600000 RX/RO (2 MiB); exchange 0x601000 (4 KiB); transfer
0x603000 (16 KiB); stack 0x608000..0x708000 (1 MiB); metadata
0x709000..0x809000 (1 MiB), no pixel/scratch ranges. First metadata page is solely
the bounded compiler private-data template. Retained State is at +4 KiB, allocator
cursor +8 KiB, bitmap +12 KiB; heap begins +20 KiB (1,028,096 bytes). Root clears
all private state on replacement. Compile assertions constrain State/header;
these sizes are finite engineering limits, not measured runtime fit.

Dependencies: embedded-websocket 0.9.5, rand_chacha 0.3.1, rand_core 0.6.4, all
without default features. The guest adds only this core and the existing compiler
memory routines. No serde, scheduler, socket, TLS or hardware dependency.
