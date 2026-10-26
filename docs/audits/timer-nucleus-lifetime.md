# Timer nucleus lifetime boundary (#6409)

The public timer helper previously accepted a repeated Play generation during
`replan`, reset its request counter, and recreated the same token. For example,
`new(7, 1) → arm A → replan(7, 1) → arm B` produced `A == B`; delivering A
could consume B as `Current`. This is an actual deterministic helper defect,
not evidence that a physical timer driver or installed product exploited it.

`replan` now returns `Result<(), TimerError>` and accepts only a strictly newer
local Play generation. A stale request leaves all pending/cancelled state and
remaining capacity unchanged. The slot owner is no longer Copy or Clone.
Valid increasing-generation replans retain their prior behavior. No production
caller was found at baseline `e18604669d993a20d4974cf6bf46397d2cac7a40`.

| Counter/state | Owner and lifetime | Finite bound / uniqueness | Retirement and late delivery |
|---|---|---|---|
| Play generation (`u32`) | One `TimerNucleus`, supplied by its admitting caller | Strictly increasing on successful replan; no wrap; `u32::MAX` is terminal for further replans | Successful replan clears pending/cancelled tokens; old generation completes Stale; refused replan changes nothing |
| Request generation (`u32`) | Current Play in that same owner | Starts0; at most `request_capacity:u16` arms, therefore at most65535 and no wrap | Arm spends one request; cancel does not refund; completion clears only the exact pending token |
| Pending slot | One move-owned nucleus, at most one pending request | AlreadyArmed refuses replacement; owner cannot be cloned/copied | Old completion cannot remove the newer generation's pending request |
| Cancelled token | One retained cancelled observation | One-shot classification, not admission capacity | First matching delivery returns Cancelled; later delivery Stale; newer Play discards it |

Permanent public-method regressions live in `timer_nucleus.rs`, under the
existing library lifecycle tests (`cargo +stable test -p conduitos --lib
timer_nucleus`). They cover same/older generation refusal, preserving pending
and cancelled custody, exactly-once completion, delayed A against B, maximum
Play generation, zero capacity and all65535 admitted requests. A compile-fail
doc example covers attempted owner reuse after move.

For a small source-bound check without a full Host build, compile that actual
production module directly, in both assertion-enabled and optimized modes:

```sh
rustc +stable --edition 2024 --test targets/conduitos/src/timer_nucleus.rs -D warnings -o /tmp/conduit-timer-tests
/tmp/conduit-timer-tests
rustc +stable --edition 2024 --test targets/conduitos/src/timer_nucleus.rs -D warnings -O -C debug-assertions=no -o /tmp/conduit-timer-tests-release
/tmp/conduit-timer-tests-release
```

Both modes passed six tests in the focused preparation. A separate scratch
public-method replay against the unchanged baseline module failed as expected
with two identical `{ play_generation:7, request_generation:1 }` tokens.
This is CPU helper proof; no native runtime, physical cancellation, DMA
quiescence, panic cleanup or complete #6409 security acceptance follows.

Tokens remain local descriptive values, not capabilities or globally unique
owner identities. Constructing a separate `TimerNucleus::new` with the same
generation can still produce equal token values. The integrating Root must
fence retired owner callbacks and establish fresh owner identity/authority;
this change does not prove cross-owner delivery or persistent Host recreation.
A surviving semantic Body grants no continuity of physical placement, lease,
Base capability or authority through this local generation. Those checked
transition and A→fault/cancel/retire→B proofs remain #6409 work, separate from
#6311/#6317/#6184 native preemption/continuation ownership.
