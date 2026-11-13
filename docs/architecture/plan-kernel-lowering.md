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

Before decoding a received assignment, the Body layer can attribute its exact
bytes with `IssuedHostCommandChallenge`. The receiver issues one non-clonable
attempt from independently retained member, issuer and Host/Boot facts, an
installed payload contract, a fresh nonce, a local deadline and a byte ceiling.
The signature covers that complete context and the payload length/digest.
Consuming a slot proves attribution only; the lowerer, full Source/Plan approval,
concrete resource admission and native possession remain independent gates.
Root must choose current offers for fresh preparation and the exact retained
reservation scope for lifecycle control. This attribution helper does not make
that authorization decision or install a transport/control endpoint.

## Selected operations under a Body Play

The optional `body-plan-binding` preparation entrance validates a selected
operation inside an ordinary Plot or selected Mask-chain fragment of the exact
Body Plan. It checks the
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

`PreparedBodyFragment` retains borrowed containment evidence after that full
preparation. Both resident computation and native assigned-operation selection
use `AssignedPlayScope` to distinguish a Plot Play from the containing Body Play.
Assigned tables retain the inner Plot Plan and fragment identities. A native
claim must name the outer Body Plan/Play and still match the independently
issued local Base capability, resource generation and envelope. A copied claim
or a descriptive assignment cannot issue that capability. The default small
Host build includes only Plot scope; receiving attributed bytes does not add a
Body preparation witness or authorize remote execution. The opt-in `delegated-preparation` entrance can instead consume an opaque
compact Body witness as described below. Root admission and start remain
separate obligations.


## Explicit delegated preparation

Host construction can select an optional preparation policy:

```conduit
preparation = {authorizer: "seed-owner", maximum_assignment_bytes: 8192, maximum_runtime_state_bytes: 8192}
```

Absence selects no delegation. The checker requires positive ceilings within
Host construction bounds, rejects unknown authorizers and duplicate declarations,
and includes selection in configuration/IMAGE identity. The ordinary Host
configuration Face exposes the selection and preserves it across edits. Changing
a target requires explicit review rather than silently discarding delegation.
No private key is authored here. The Seed resolves `seed-owner` to its exact
pinned Body/issuer context, independently of a request's self-description.

Rust Root generation emits the checked policy as a typed constant, including
`None` when absent. It does not add a runtime Host-configuration parser.
`check_host_preparation_delegation` is an early, allocation-free policy guard
over an already attributed command. It rechecks current membership, receiving
Host/Boot/profile/generation, installed IMAGE/configuration, exact issuer
Host/Boot/generation/key, payload contract, deadline and byte ceiling. The
runtime-state ceiling must still constrain subsequent assignment validation.

Success permits considering preparation; it is not approval of the particular
Plan, a reservation, native possession or kernel start. The receiver must use
its installed policy, never a policy supplied by the command. Connecting the Root receive/reserve/start protocol and physical provider
handover remains the #6770 delivery obligation. Construction and guard tests
alone do not establish a playing constrained Host.

### Compact approved Body fragments

The authoring-side `generate_prepared_body_fragment` takes a fully checked
`PreparedBodyFragment`, including its exact containing inner Plan. Containment
covers ordinary Plots and selected Mask-chain Plans; duplicate containment
refuses. It uses the existing Fore/native projection generator and assigned
format. Its current profile still refuses network Lines and fused activations.
It creates no signature or execution permission.

The document is self-delimiting borrowed Postcard scope metadata followed by
unchanged assigned bytes. It travels inside the existing attributed command and
bounded-document machinery. The metadata retains Body, Wake, outer Body Plan and
Play, inner Source/checked/expanded Plot/Plan/fragment, Host/Boot/generation and
an exact projection digest. The Body Play digest keeps its existing identity
law; the small receiver checks it without constructing a concatenated buffer.

`receive_delegated_body_fragment` consumes an attributed command, first checking
the independently installed delegation and current Seed/member/receiver facts.
It checks exact scope and projection bytes, then uses Core's existing decoder
with receiver-local requirements, computation/Fore pins and actual profile
ceilings narrowed by policy. Success returns a borrowed, opaque
`DelegatedBodyFragment`; public metadata cannot construct one. It is preparation
evidence, not a reservation, Base capability or active Play.

`AssignedPlayScope::Delegated` binds the containing Body Plan/Play and the
validated assigned header. Core checks the candidate records against the header
digest, preventing substitution of different tables with the same descriptive
IDs/counts. Both contiguous and borrowed-record inputs use this rule. The small
Host need not parse Conduitese or re-run the rich planner. Actual resource
admission, one-attempt custody, old-execution retirement and provider handover
remain Root duties. CPU receive/bind/kernel/retirement proof and Cortex-M0+
compilation do not establish physical MCU execution or native effect authority.

### Exact plain resources and complete local admission

Preparation document schema 2 also carries borrowed, self-delimiting plain
resource descriptors: numeric node/resource, original sealed placement ID,
pool/class and exact units. These preserve the rich Plan's identities across
numeric lowering. They do not reconstruct a capability catalog. The canonical
factory refuses protected, compute-lane or content bindings rather than erasing
their contracts into this plain profile. Plain binding units are also the exact
plain requirement units under Core's existing resource law.

The receiver checks descriptor bounds, unique node/placement correspondence,
exact numeric record coverage and units without allocating. It retains a compact
pin of the independently checked receiver profile. Resource preparation rechecks
the actual owner's Host/Boot/profile/generation and installed plain pools;
public strings do not create a resource. Only then does it allocate the exact
Core request/item capacities and call `ResourceAdmissionOwner::admit_batch`.
There is no second reservation ledger. Core remains responsible for current
Ready observations, existing reservations, exact binding satisfaction and atomic
capacity refusal.

Each approval commits to the complete local fragment set through a count and
canonical digest over its fragment identities, covering ordinary Plots and
selected Mask chains on that exact Host/Boot/generation. The batch entrance
requires that complete set and one exact containing Body Plan/Play. Missing,
duplicate or mixed-scope fragments refuse before resource mutation. The receiver
checks the set digest without allocating or retaining another ID catalog.

This reserves resources only. Root must still admit the whole preparation and
execution workspace, enforce one-attempt/retirement custody, transfer actual
initialized providers and start the kernel. Cleanup uses the retained original
Body Plan/placement reservations. Kernel terminal evidence must precede returning
resident loans or releasing live execution resources. The component API does not
make its caller's ordering or physical execution automatic.

## Resident backing across Plays

Generated resident libraries can lend a selected Back through `borrow_entry_N`.
The fixed kernel owns an exclusive pointer to that entry's existing backing,
not another inline evaluator. Exact Source/Fore/artifact and active-Play checks
still precede the loan. Nominal generated loan types retain the exact owning entry even when engines
have identical Rust layouts but different program or temporal contracts.

Root first drains or cancels the kernel and uses its existing `try_retire`
boundary, retaining its old values and Signs. The returned loan's
`finish_retired` clears stateless scratch and restores inventory eligibility;
it does not release resource reservations, publish a Host generation or grant
another Play. Uncommitted closure, live execution and abandoned loans cannot
silently republish the implementation. The next Play still needs fresh checked
binding and admission. These mechanics do not migrate semantic State or prove
physical bus quiescence. Root's full workspace and actual execution remain
independent custody obligations under #6341 and #6340.

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

## Assigned runtime-state declaration

Assigned format `CNDAP002` uses bytes 12–15 of its unchanged 125-byte header
for a little-endian `u32` runtime-state declaration. This replaces the former
`u16` declaration and reserved word; old format images are rejected. The
encoded-image bound remains `u16`, and every receiver checks the full state
declaration against its independently installed policy, including the checked
sum across a workset. A wider declaration grants no physical memory: concrete
retained backing, preparation overlap and execution resources still require
ordinary admission before Play.
