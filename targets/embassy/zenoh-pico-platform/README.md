# Zenoh-Pico Embassy socket owner

This `no_std` target crate services an actual caller-owned Embassy `TcpSocket`
using bounded nonblocking passes. It supplies no executor, socket slot, TCP
buffers, worker, allocator, credentials, discovery or Body authority. Call `poll`
from the socket's executor, yield between passes, and run the blocking Zenoh C
worker separately. Reserve the socket, RX/TX backing, staging bytes and worker
stack before starting. No lock is held across a network operation.

The shared `conduit-zenoh-pico-platform` handoff stays dependency-free. This
separate workspace pins the same Embassy source as the Pico firmware without
introducing its embedded time driver into the Linux workspace's dependency graph.
Only this adapter depends on Embassy; other native targets need not carry it.

One pass checks close/deadline, observes the same connection attempt, and moves
at most one staging chunk each way. A failed SYN is not silently restarted by
calling `try_connect` again. `try_write` supplies an exact accepted prefix;
`WouldBlock` permits retry because it accepts zero bytes. Close aborts the actual
socket before releasing the generation. Buffered received bytes remain readable
after EOF/failure. Recorded endpoint metadata comes from the actual socket.

Compile and lint:

```
cargo +stable check --locked --manifest-path targets/embassy/zenoh-pico-platform/Cargo.toml --target thumbv6m-none-eabi
cargo +stable clippy --locked --manifest-path targets/embassy/zenoh-pico-platform/Cargo.toml --lib -- -D warnings
```

These checks are not linked firmware, a working C PAL, admitted worker backing,
MCU flash/RAM measurements, NAT recovery, or durable history proof. The actual
firmware adapter must expose endpoint snapshots under its own synchronization,
validate C slot/generation, and supply real monotonic time, entropy and an
explicit finite arena. Transport progress cannot acknowledge recording custody.

`SocketWorker` is the safe worker-side API for those C bindings. Its caller
supplies a real monotonic clock and bounded worker sleep. Reads/writes return
accepted prefixes immediately; nonblocking operations never sleep. A close has
an explicit finite deadline, and a timeout leaves the socket quarantined until
the actual owner aborts it. Failed unpublished connection attempts request the
same retirement. Readiness includes buffered failure/EOF without consuming it.
No thread, raw C export, clock, allocator or entropy provider is installed by
constructing this value. Worker scheduling fixtures test these contracts;
`socket_owner` separately exercises actual Embassy TCP with finite packet queues.

`CArena<ALIGN>` reuses the Pico firmware's `linked_list_allocator` over an explicit
static slice. It has no global fallback or extension. Select ALIGN from the C
target's `max_align_t` contract and check it in the C build. Its size prefix keeps
returned pointers aligned; null/zero/overflow and failed realloc obey the C
ownership contract. Free wipes the released block. Requested payload and actual
allocator use have separate current/peak counters, including realloc overlap.
The target must partition existing admitted backing before construction; this
helper does not enlarge a grant or install a global allocator.

The pinned upstream 1.10.2 handshake send path leaked a partially allocated wbuf
on OOM. The retained candidate patch in `patches/` adds the missing clear before
return. A Linux session using the actual Rust arena reproduced eight consecutive
4 KiB failures accumulating 320 payload bytes/448 allocator bytes; with the patch
all eight refusals returned to zero. Successful repeated opens also cleaned to
zero. This is a local upstream-patch evaluation, not an accepted upstream fix;
the adapter does not silently fetch or patch a library during its build.

`Callbacks` and `export_zenoh_pico_callbacks!` bind the C PAL in `c/platform.c` to
that worker, arena and a target-supplied `CallbackPlatform`. The C declarations
and finite socket types live in `include/`. The target owns context initialization
and exclusive single-worker access. Never hold a critical section, interrupt
mask or cross-core lock across a callback: worker waits need the network owner
and sampler to keep running. Only individual bridge/snapshot accesses lock.
Entropy failure parks/retires the isolated worker without substituting bytes;
its backing cannot be reused before retirement. Wall time may be unavailable.
Endpoint strings come from an actual generation-matched socket snapshot.

The raw ABI still requires valid caller-owned extents and aligned u32 outputs;
null, declared-size and slot checks cannot authenticate arbitrary C pointers.
Provide disjoint local/remote output buffers. Tests instantiate and invoke the
exported ABI using explicit fixture providers, not hardware entropy or clocks.
Production must supply those resources and verify `CONDUIT_Z_ALLOC_ALIGNMENT`
against its Rust arena. The C PAL deliberately fails to compile without that
alignment definition. It is compiled separately with the pinned Zenoh headers;
the Rust build neither downloads a library nor installs callbacks automatically.
