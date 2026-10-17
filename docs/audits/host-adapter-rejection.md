# Host adapter rejection scan bound

Development slice of #5231, 17 October 2026.

`next_host_request_matching` now counts down the already maintained number of
undispatched requests. Once an adapter has rejected every such request, it
returns without scanning the remaining reserved slots. Dispatched entries and
holes do not consume that count. Accepted selection, callback order, request
identity, Signs, and the cursor on a rejected poll retain their existing rules.
No new scheduler storage or allocation is introduced. Dispatch implementation
lives in `scheduler/host_dispatch.rs`, reducing the oversized scheduler root.

The 4,096-slot production scheduler fixture with one request at slot zero now
visits 1 slot from cursor zero (previously 4,096), 2 from cursor 4,095, and still
4,096 from cursor one. The last case explicitly retains the linear worst-case
bound. Callbacks run exactly once for that request; rejection leaves the cursor,
undispatched count, and Signs unchanged. Subsequent dispatch succeeds; an empty
eligible set performs zero probes and callbacks.

The existing 2,048-Host cancellation/completion/rearm fixture now fills all calls
before dispatch, rejects the entire batch in exact node order, and verifies
2,048 probes rather than the 4,096-slot reservation. It then dispatches normally
and exercises both completion and global-cancellation paths, including its
existing zero-allocation and exact Sign assertions.

Local validation: full kernel tests with `alloc`, repeated with
`alloc,linear-ready-reference-test`; strict kernel Clippy across all targets;
formatting and diff checks. Temporarily restoring the old full-reservation
traversal makes the new rejection regression fail at 4,096 versus 1 probes;
the production change is restored afterward. These are deterministic work and
semantic tests, not elapsed-time speedup measurements. No CI or stable-release
acceptance is claimed; #5231 remains open.
