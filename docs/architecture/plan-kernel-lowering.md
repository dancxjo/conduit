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


## Resident computation assignment (development)

Make may prepare an immutable structured-expression realization before planning.
`PreparedResidentExpression` derives its Kind/Fore from the checked program and
Native owners. Preparation establishes eligibility; it creates no initialized
Back, offer, reservation or authority. Its artifact identity hashes the base
generated realization source, not the linked Seed or a claim of flash placement.

Binding requires a verified sealed Plan with the same Source document, exact
Kind/Fore/configuration/laws, resident profile, implementation and artifact.
The pure profile refuses Host Calls, Base/authority bindings and realization
overrides. The generated selected factory compares required work with the
exact lowered Step grant. Generated-source capacity applies after the selected
wrapper as well as before it. Initialized-provider and complete Host resource
admission remain separate mandatory steps.
An image or template made against one Boot does not authorize a later Boot;
deployment still requires fresh initialized-provider evidence and exact binding.

The existing assigned configuration record supports an opt-in resident form
with discriminant `4`: node u16, key identity (16 bytes), discriminant u8, logical
canonical-text length u32, and SHA-256 (32 bytes), all integers little endian.
This is a 55-byte payload in the existing framed record, not another protocol.
Its trusted inventory must be installed with the resident image; receiving a
digest cannot supply that inventory or manufacture executable code.

Both contiguous and borrowed-record decoders enforce the same exact node/key/
logical-length/digest inventory, uniqueness, completeness and finite logical
artifact limits. Canonical text mode retains its existing checks. The logical
length uses u32 so a large program can be resident without requiring its text
to fit a u16 framed payload. Encoded-image, runtime-state, queue and execution
grants are unchanged. Structural record inventory and actual native capability
possession retain their independent admission roles. This metadata admission
is not proof of a Ready Back, Seed installation or physical Play.
