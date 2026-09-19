# plan-to-kernel lowering

`conduit-plan-lowering` owns one narrow pre-play boundary:

```text
verified PlanFragment with rich identities
        -> exact capacity validation
        -> numeric kernel tables plus KernelIdentityMap
```

The package does not plan, schedule, execute semantic implementations, adapt
host effects, or own product lifecycle. `NodeId`, kernel `PortId`, `CordId`, and
the other compact numeric identities stay below this boundary. The retained
`KernelIdentityMap` is the exact correspondence back to plan identities used by
hosts, signs, diagnostics, and presentation.

## Fixed storage profile

`FIXED_KERNEL_STORAGE_PORTS_PER_NODE` is the backing width of the current
allocation-independent `NodeSpec` tables. It is a fixed-storage implementation
fact, not a semantic limit and not planner policy.

A host selects a `KernelStorageProfile` before lowering. It may select a
narrower per-node width than the backing tables. `lower_plan_fragment_for_profile`
refuses a plan that exceeds that selected profile before kernel construction or
play start, reporting the exact placement, port direction, required width, and
available width. Profiles cannot request zero width or silently exceed the
compiled fixed backing.

Adding a future fixed storage layout means adding an explicitly owned profile
and matching kernel table representation. It does not authorize dynamic growth
after play start.
