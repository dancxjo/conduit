# Hosted model transport cancellation

Issue [#6877](https://github.com/dancxjo/conduit/issues/6877) owns this selected
provider contract. It extends the existing asynchronous `llm/generate-flow`
worker; it does not add a scheduler, retry policy, or model authority.

On Unix, a prepared Ollama generation worker resolves its curl executable and
owns one cancellation token, one reusable input/output slot, and at most one
transport. The existing hosted process driver checks cancellation between
nonblocking pipe operations at a 2 ms polling interval. Cancellation kills the
owned process group, closes input, allows at most 100 ms for pipe retirement,
and reaps the child before returning the slot. Worker retirement requests this
cleanup before joining the thread. An idle prepared worker and its admitted
buffers remain reusable until retirement; repeated cancellation does not create
additional workers or requests.

The deterministic Unix profile uses a **two-second test ceiling** from a
cancellation request to observed local transport/worker release. This is a
measured hosted transport bound under responsive OS scheduling and ordinary
process termination, not a hard real-time guarantee against an unresponsive
kernel, uninterruptible process state, or hostile escaped descendants. Killing
curl closes this client's HTTP connection; it does not prove server-side GPU
work has stopped, unload shared model residency, or terminate the Ollama server.

The raw generation response retains the previous 16 MiB wire ceiling. This
client buffer and its 4096-byte diagnostic buffer are separate from the model's
admitted weight-memory value. Request serialization has an 8 MiB process-input
ceiling, above the finite semantic input and its JSON representation. Wire
overflow fails before decoding; retained prefixes cannot become successful
model results. The explicit larger-output process entrance admits this client
buffer without increasing the ordinary or funded process entrances' existing
131116-byte ceiling. Inventory and initialization keep their existing transport
and 16 MiB inventory policy.

Cancellation publication remains owned by the existing kernel Host Call and
installed pending-call owner. The worker's cancellation flag discards late
output even when success races cancellation. Its token is reset only after the
single slot returns. Replacing a Play retires and joins the old worker before
preparing a new worker with a distinct token. A retained old token cannot cancel
the replacement. These checks support the scoped
[#6409](https://github.com/dancxjo/conduit/issues/6409) lifetime obligation; they
do not establish every provider, fault, counter-exhaustion, or isolation case.

At the local-model Host Call boundary, successful completion and cancellation
retain their existing dispositions. Failed calls distinguish implementation
failure (detail 53), provider loss (54), invalid structured result (55),
transport timeout (56), and response capacity overflow (57), using the existing
`host_call_failed` failure class for timeout and wire capacity. Curl's timeout
exit and the parent request deadline both produce the timeout detail. A timeout
cannot become a fabricated provider-loss episode.
The process report records whether the wire bound established retirement;
buffered bytes drained during cleanup cannot rewrite an already established
cancellation, timeout, or unrelated provider loss into a capacity failure.

The durable pool-member wire consumer preserves its existing codes: refusal
(70), implementation/structured-result failure (71), cancellation (72), semantic
output payload limit (73), and provider loss (74). Transport timeout (75) and raw
response capacity overflow (76) fail each active session separately from that
semantic payload limit. Failed operations publish no result payload, including
any bytes retained by the adapter; cancellation retains the cancelled wire
terminal. The deterministic product session tests exercise actual planned
session machines and encoded failure/terminal frames with synthetic model
terminal inputs, rather than invoking a model provider.

The generic model offer still reports `cancellation_supported: false`: only this
prepared Unix generation worker has the selected transport behavior. Synchronous
model operations, initialization, inventory, text streaming, and other operating
systems retain their existing limits and must not inherit a stronger claim.

The owning std tests exercise actual curl children against a fenced loopback
HTTP peer, including pre-dispatch cancellation, stalled/partial responses,
timeout, provider loss, overflow, repeated cancellation, and replacement custody.
The Pete cadence test keeps reaction and attention progressing while curl is
stalled, cancels A without an episode, then admits a fresh Play on the same Host
and verifies only B's responses. These are deterministic hosted transport and
kernel proofs, not live-model quality, physical robot, or stable-release proof.

The tests remain in `cargo xtask check` and its std Host shard. The cadence
regression is also included in `cargo xtask check pete-cognition`.
