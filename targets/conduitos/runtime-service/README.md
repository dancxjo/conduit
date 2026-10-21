# Native workset runtime service

This independently linked x86_64 guest runs the canonical fixed scheduler in
`conduitos-runtime-core`. Each Root command enters the independently owned
Runtime service profile, executes a single command and returns through the
existing service gate. Root retains the authoritative reviewed Host effect
bindings, Body/Play lifecycle and canonical application Host Call adapters.

The Runtime profile has an RX/RO image, a 4 KiB exchange, a 16 KiB transfer
window, a 256 KiB stack and 1 MiB private metadata. The retained scheduler begins
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
