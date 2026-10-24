# Original storage failure through the native worker

The file publication completion exposes the exact Failure already completed into
its original kernel request, alongside the domain refusal. This does not change
request execution, authority, retry behavior or observer commit rules.

A new CPU native-worker case pre-publishes the second selected immutable
generation through the real file operation. The first sample commits; the second
write refuses. The file kernel commits its BackFailed result, and the sink
propagates that same Failure to native execution. Native cleanup succeeds and
both acceptance and durable watermarks remain one. The second observer remains
empty, so a complete catalog refuses. Independent read owners verify the first
sample and the old second publication are unchanged. The attempted candidate
is retained as a failed external effect; no false rollback is claimed.

Both assignment representations exercise this case. All seventeen Root tests,
the public checkpoint/refusal/restart suite, strict Linux lint and formatting
pass. The existing Root harness dead-code allowance for included target modules
remains explicit. The sample/write/refusal phase has zero Rust allocations.

The finite 960-reading success proof remains. Admission, Line, clock, store
placement and grants are CPU fixtures; aggregate Host reservation, checkpoint
allocation closure, physical deployment and combined MCU admission remain
unproven. No new hardware image was flashed.
