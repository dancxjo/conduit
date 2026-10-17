# Prepared quiescence emission bound

Development slice of #5231, 17 October 2026.

Preparation now records the exclusive upper cord index containing a quiescence
track, rather than merely whether such a track exists. Both ordinary and boxed
preparation derive it from the validated active prefix. Runtime emission keeps
its original ascending cord order, admission pass, shared value transaction,
queue writes, ready wakes and Signs; both scans stop at this bound. Arming still
uses exact source routes. The zero bound preserves the absence fast path.

No runtime transition maintains the bound: cord specifications are immutable.
The cached field changes from a bool to a usize; no per-cord index, dynamic
allocation, or extra ready-set storage is added. Layout impact depends on target
alignment and is not asserted here. Preparation searches backward to find the
last quiescence track; worst-case preparation still visits every active cord.

The 128-node fixture has 127 active cords, two quiescence cords at indices zero
and one, and quiescence-marked inactive padding at index 127. Emission now visits
4 cord entries across its two passes instead of 254. A subsequent unarmed poll
visits 2 instead of 127. Both quiescence queues receive exactly one value, the
payload queue stays empty, arming flags clear, and repeated polling adds no Sign.
An exhaustive preparation test checks all 256 track masks at every active prefix
length from zero through eight. Existing ordinary and boxed tests retain the
inactive-padding and absent-track checks.

This is a reserved-tail optimization: quiescence near the end of the active
prefix still requires linear scans, and a wholly inactive armed set does not
receive a separate runtime index. Probe reductions are deterministic work
observations, not measured elapsed-time speedups. The broader #5231 performance
and stable-acceptance requirements remain open.

Validation: full kernel suites with alloc and with the linear ready reference;
strict all-target kernel Clippy; formatting and diff checks. No CI dispatched or
awaited for this slice.
