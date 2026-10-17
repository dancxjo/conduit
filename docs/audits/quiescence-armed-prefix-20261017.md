# Quiescence emission stops after the last armed edge

Local #5231 implementation slice, 17 October 2026.

Quiescence emission previously traversed the entire prepared quiescence cord
prefix twice when any edge could emit. In a valid 128-node chain with 127
quiescence cords, arming only cord 0 incurred 254 cord visits. The baseline
fixture was run before changing the algorithm and retains its test log.

Both passes now snapshot the existing exact armed-edge count and stop after
observing that many armed flags. The count decreases for every observed flag,
including nonempty queues and completed sources, before checking eligibility.
Those ineligible flags remain armed in canonical scheduler state. The emission
pass uses its own snapshot so disarming emitted edges does not prematurely
stop traversal. Sign capacity and shared-value fan-out remain preflighted
before publishing any edge; the original ascending-cord order is preserved.
No extra runtime field, allocation, storage reservation or index is introduced.

The same sparse fixture now requires two visits. It also exercises a full early
queue, a completed middle source, an eligible final edge, repeated idle polling,
drain/re-emission and global cancellation with full value reclamation. A separate
full-scan oracle checks 64 combinations of unarmed, eligible, full and completed
edges at positions 0, 63 and 126, including exact emitted Sign order, first source,
remaining armed counts, duplicate suppression and final reclamation.

Both kernel selectors pass 238 local tests. Strict kernel Clippy, workspace
formatting and the portable no-default-features check pass. Raw logs and source
hashes are retained under `evidence/quiescence-armed-*`. The focused internal
fixture can be selected with `cargo +stable test -p conduit-kernel sparse_armed_quiescence -- --nocapture`; ordinary public validation remains `cargo xtask`.

This eliminates an unarmed suffix, not all graph-size work: scans remain linear
through the highest armed cord. Each visited cord now has an extra local
remaining-count guard. No dense/tiny elapsed-time regression or full Plan/Play
speedup is claimed; performance timing and parent acceptance remain open.
No CI was dispatched or awaited.
