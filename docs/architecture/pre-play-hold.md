# Optional pre-Play HOLD

Issue [#646](https://github.com/dancxjo/conduit/issues/646) adds one optional
core gate to the Body lifecycle:

```text
BODY -> WAKE -> PLAN -> [optional HOLD] -> PLAY
```

HOLD is a state of an awake Wake. The exact immutable Plan already exists, but
no Play has started and no `ActivePlayId` exists. HOLD is not a UI breakpoint,
not a pause of an active Play, and not Lull.

## Held contract

`conduit-body` retains a bounded `PlanHold` containing:

- the complete exact Plan, through which selected Hosts, Boots, Bases, Lines,
  resources, capabilities, implementations, limits, and authority bindings are
  inspectable when applicable;
- a finite ordered `PlanningBasis` of exact Clue identities for every relevant
  planning Sign;
- an exact hold reason and source;
- the explicit authority contract and grant required to release the hold;
- whether the same policy must hold a replacement Plan again.

`Wake::inspect_hold` returns those facts plus whether a supplied current basis
still exactly matches the held basis. Visibility and reachability provide no
release authority.

## Release law

`Wake::release_hold` performs the gate in this order:

1. validate the Wake, held Plan, finite basis, policy, and supplied identities;
2. require the exact `conduit.authority/release-held-plan@1` contract and grant
   named by the hold;
3. compare the complete current planning basis with the held basis;
4. only when it still matches, bind the first `ActivePlayId` and enter
   `Playing`;
5. when it differs, create no Play identity, mark the Plan `Invalidated`, and
   enter `AwaitingReplacement`.

The Body lifecycle records `HeldPlanReleased` or `HeldPlanInvalidated` as
distinct machine-readable events. It does not invoke a planner or a platform.
The ordinary planner supplies a separately sealed replacement Plan. If the
hold policy persists, direct replacement admission fails with `HoldRequired`;
the replacement must cross `plan_held` and becomes visibly `Held` again.

## State paths

```text
AwaitingPlan -> plan_ready -> AwaitingPlay -> play_started -> Playing

AwaitingPlan -> plan_held -> Held
Held -> authorized release + same basis -> Playing
Held -> authorized release + changed basis -> AwaitingReplacement
AwaitingReplacement -> replacement plan_held -> Held     (persistent policy)
AwaitingReplacement -> replacement plan_ready -> AwaitingPlay (nonpersistent)
```

Calling `play_started` while held fails. Calling `plan_held` while already
playing also fails; HOLD therefore cannot masquerade as runtime pause. Lull is
an explicit, separately recorded lifecycle transition and is never caused
automatically by waiting in HOLD.

## Bounds and stop line

Planning basis storage is capped by `MAX_HOLD_BASIS_SIGNS`; empty, duplicate,
oversized, or invalid identities fail closed. Wake Plan history and Clue history
retain their existing finite bounds. The Plan is cloned only while the Wake is
already awake and before Play; active execution remains outside this module.

This slice adds no UI, planner, authority issuer, platform adapter, ambient
controller, active-Play pause, or automatic Lull behavior.
