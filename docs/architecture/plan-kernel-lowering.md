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


## Cooperative private Step work

The fixed lowerer grants one base unit plus one unit per input, output and
Host Call. Private computation needs an additional explicit reservation: a Host
offers a finite pool of resource class `kernel/cooperative-step-work@1`, and a
Back requires a bounded number of its units through the ordinary resource
contract. One unit grants one cooperative private-computation fuel unit per
Step. These are neither elapsed-time units nor a preemption mechanism.

Only selected, Plan-sealed bindings of that exact class supplement the
kernel-visible grant. Absent and unrelated resources add nothing. The lowerer
retains those bindings as kernel resources, adds their units with checked
arithmetic, and refuses grants that exceed the kernel's `u16` fuel bound.
Zero units and bindings carrying protected, compute-lane or content metadata
refuse. No pool or reservation is inferred from a program, firmware image or
implementation name. Changing a binding changes Plan identity.

The ordinary planner accounts for this pool with all other resources and
refuses missing or insufficient capacity. The installing Host must still
reserve and validate its selected resources before Play and check that each
Back's required private work fits its exact grant. A Back spends private work
through `StepIo::consume_fuel`; unspent reservation is not authority for IO, and
cooperative charging is not hostile-code confinement.


## Resident scalar expression Back

`StaticScalarExpressionBack` installs a checked immutable scalar image through
`PreparedExpressionBack`'s existing transactional lifecycle. Its evaluator keeps
only fixed result storage and lends code/literals from the resident image.
It spends its declared private work through `StepIo` before publishing output or
consuming input. Value, closing Flow, pressure, rejected transaction, cancellation
and reusable-session behavior share the existing implementation.

Construction is allocation-free and refuses unsupported temporal profiles or
unrepresentable work. It does not publish a provider, reserve its pool, admit a
Body Plan or authorize physical effects. Complete Host workspace admission,
actual code placement, current offers and execution remain separate obligations.
