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
