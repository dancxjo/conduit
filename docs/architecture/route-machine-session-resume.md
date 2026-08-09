# Deterministic Lines and bounded session resume

Issues #501 and #618 define runtime policy over the exact Lines sealed by the
active Plan. `LineMachine` copies that finite ordered set once. Later
`LineAvailabilitySign` values may change only the availability of the exact
`LineId` and lower binding already admitted. The first Ready Line wins. An
unknown Line or binding is rejected; an exhausted set yields
`LineDisposition::Unsatisfied` with `replan_may_be_requested`, but the machine
never invokes a planner or changes the Plan.

`LineUpdate` retains the exact availability Sign, previous `LineId`, and either
the selected `AdmittedLine` with `same_plan_continues` or the explicit
unsatisfied result. Observation change, Line selection, and replanning remain
distinct machine-readable events.

## Finite reconciliation

`SessionCheckpoint` contains only the next sequence, at most one Offered or
Accepted transfer, and whether input closed. There is no replay history.

`SessionMachine::checkpoint_offer` binds that finite state to the logical
session. `resume_with_attachment` requires an already admitted Line attachment,
the unchanged logical identity, and one peer checkpoint. It yields `Continue`,
`ReplayOffered(n)`, `AwaitReplay(n)`, or `AdvanceDelivered(n)`. Contradictory,
stale, failed, terminal, closed, or different-session combinations fail closed.
Peers repeat Hello/Ready for the exact new Line attachment before application
traffic resumes. This proves only these bounded transitions, not general
exactly-once delivery across physical failure.
