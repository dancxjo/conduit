# Quiescence arming follows exact source routes

Development slice for #5231. When a Play had any quiescence cord, arming one
active source scanned every active cord to locate that source's quiescence
outputs. Plan validation already proves every local-source cord appears in its
exact source port's sealed route, and every routed target has that same source,
is active, and appears only once per port.

Arming now visits the node's admitted output port table and its routed targets,
setting the same quiescence flags only on quiescence tracks. It borrows the
existing immutable route table and adds no storage or allocation. The existing
no-quiescence fast path remains. Cost is bounded by port capacity plus source
outdegree rather than the active graph's cord count. Port-table probes still
cost work even for a node with no outgoing targets.

A native fixed-storage regression admits 128 nodes and 127 cords. One source
has two quiescence targets and one payload target; other cords are remote
ingress, and a quiescence-marked padding entry is inactive. Before the change,
arming that source performed 127 cord inspections; afterward it performs three.
Arming a node with no outgoing route performs zero cord inspections. The test
also checks that only the two quiescence flags are armed and padding stays inert.
This is deterministic work evidence, not a measured wall-clock speedup.

Quiescence edge emission still scans active cords when the whole Play becomes
quiescent; this change only removes graph-wide scanning from source arming.
It does not change edge ordering, Sign reservation, value retention, wakeups,
terminal suppression, or the meaning of quiescence. Existing epoch/rearming and
terminal/failure/cancellation tests exercise those semantics locally. Broader
production Plan/Play performance and parent-issue acceptance remain open.

Local validation: alloc and linear-ready-reference kernel suites each passed
190 tests; four focused fixed-only quiescence tests passed; formatting, strict
alloc all-target kernel Clippy, and diff whitespace checks passed. No CI was
dispatched or awaited, and no physical-device or stable-release proof is claimed.
