# Browser input sequence retirement

The pointer observation ABI carries a u32 sequence. The button ABI converts an
exact JavaScript Number to a u64, so its producer retires at MAX_SAFE_INTEGER.
The final representable value is emitted once; the following admitted physical
observation refuses `IdentityCapacityExhausted` before pointer capture or routing
can mutate held input ownership. The pure `advanceInputSequence` policy tests
exercise those two exact boundaries without mutating an acquired input owner.
They are permanently registered as `check.browser.human-input-sequence` in the
existing `cargo xtask check browser` Node entrance.

The input owner also fences DOM handlers after terminal refusal. Page loss is a
realistically reachable terminal control: the Chromium fixture loses the page,
then performs real pointer and keyboard actions and observes no further routing
calls or pointer values. Pending button and keyboard requests retain `PageLost`.
Focus loss remains recoverable by the existing focus transition; a later blur/focus pair cannot downgrade PageLost and revive the owner. The regression first failed with an accepted observer after that pair, before terminal absorption was corrected. A failed button
stream no longer captures new button routing ownership while unrelated pointer
observations may continue. Counter capacity is checked before routing callbacks
and DOM pointer capture; counters advance only for observations actually routed.

Proof remains scoped: two deterministic policy cases and the thirteen existing
real Chromium DOM tests, one pinned Chromium worker, zero retries. No Rust/WASM
build, runtime counter setter, billions-of-events test, full WASM forced-limit
proof, bearer authority exploit, hard isolation or complete #6409 acceptance is
claimed. The broad public browser suite and its hosted CI selection are separate
from these focused Node/DOM results. Original prepared proof at d227f2a3 remains
historical evidence; the refreshed branch retains that commit and current dev
through a normal merge rather than substituting the old result for new tests.
