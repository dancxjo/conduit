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
hosts, signs, diagnostics, and face.

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

## Selected operations under a Body Play

The optional `body-plan-binding` preparation entrance validates a selected
operation inside an ordinary Plot fragment of the exact Body Plan. It checks the
current Wake/workset and Body Plan seal, the Body Play identity, exact fragment
membership, Host/Boot, and the independently retained possession scope. The
claim names the containing Body Plan and its one body-wide Play. It does not
invent a separate child Play to satisfy a fragment identity check.

The selected Plot fragment retains its own identity and exact numeric lowering.
Both entrances share the same private operation, Fore, configuration, Host Call,
resource, authority and table correspondence checks. Binding returns a numeric
node; it starts no operation, changes no lifecycle state and issues no authority.
Actual Host preparation, resource admission, execution and retirement remain
separate. The existing fragment-Play entrance and its canonical active-Play law
remain intact. Body validation is an optional dependency, disabled by default.
