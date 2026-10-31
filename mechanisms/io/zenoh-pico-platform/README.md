# Zenoh-Pico platform backing

A finite socket handoff between a blocking Zenoh-Pico worker and its actual
asynchronous network owner. This crate supplies no Zenoh implementation, socket,
thread, allocator, clock, credentials, discovery, Body membership or execution
authority. It is being integrated in the Continuing Thermometer's existing
Line-base evaluation, not installed as an alternate runtime.

The caller supplies fixed RX/TX capacities and protects short mutable accesses
with its existing cross-core mutex. Its Embassy task retains the real socket;
no executor-local socket reference crosses cores. No allocation occurs in this
crate. Connection deadlines are local monotonic preparation time, never sample
or wall time. Close must be taken and completed by the actual owner before the
slot is reused. Generation checks reject late connection/read completions.
One-use send tickets prevent duplicate or oversized completion from consuming
another publication in the same socket generation. Buffer custody and partial
socket-write completion are not durable recording acknowledgments.

The network owner must cancel in-flight operations before retiring a socket,
submit each receive completion once, and retain any unaccepted receive suffix.
Cancel a send ticket for retry only when zero bytes were accepted; unknown
partial acceptance requires faulting and closing that socket generation.

Run `cargo xtask check zenoh-platform`. That suite checks this state machine's
pressure, partial transfer, EOF/fault, cancellation, deadline and stale completion
behavior. It does not prove a working C PAL, MCU footprint, a Conduit protected
Line, NAT recovery or local sensing/display continuity. Those require the actual
platform worker, admitted stack/arena/network backing and device experiments.
