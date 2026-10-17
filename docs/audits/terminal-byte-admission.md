# Abnormal terminal byte admission before commit

Development fix for #5231 terminal/pressure preservation, 17 October 2026.

An abnormal Step could previously commit its Back and mark the source complete
before `abnormally_terminate_outputs` discovered that the terminal value alone
exceeded a target abnormal-terminal cord's byte capacity. The failed publication
left an irrevocably completed node without its terminal value.

The existing precommit terminal publication pass now checks the terminal byte
length against every matching abnormal-terminal target. It returns the existing
`QueueCapacityExceeded` refusal before Back commit, completion, queue writes,
value allocation or closure publication. A queued-input consumption cannot make
this particular value fit: it exceeds the cord's entire byte budget. Normal
closure uses a zero-byte value and retains its behavior. Route and Sign ordering
on admitted publication are unchanged; no additional pass or storage is added.

The regression uses two admitted remote abnormal-terminal cords with three-byte
budgets. A four-byte terminal refuses with zero Back commits, source incomplete
and ready, empty value storage/queues, and no closures. The pre-Step fuel Sign
remains observable. Retrying the same source with a two-byte terminal commits
exactly once, completes the source, and publishes the same two-reference value
to both cords. Temporarily disabling the new admission check reproduces the
previous failure: committed count one instead of zero. The fix is restored
before final validation.

Full local kernel tests pass with alloc under both packed and linear ready
selectors; strict all-target kernel Clippy, formatting and diff checks pass.
This fixes oversized-value refusal only. It does not claim general rollback for
all terminal storage failures or queue occupancy conflicts, nor stable release
acceptance. No CI dispatched or awaited; parent #5231 remains open.
