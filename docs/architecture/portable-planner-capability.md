# Portable planner capability

Issue [#468](https://github.com/dancxjo/conduit/issues/468) makes planning an
optional host capability. It does not create a planner host class, coordinator,
planner service, or second source of plan truth.

The boundary is:

```text
portable planning inputs
  checked form + target host advertisements + placement/policy
  + base availability + authority grants + observed link bindings
                              |
                              v
optional advertised planner profile and finite admission limits
                              |
                              v
shared deterministic planner contract -> exact Plan and PlanFragment values
```

`HostAdvertisement::planner_capabilities` is empty for a host that cannot plan.
Each offer names a profile and exact maxima for host advertisements,
operations, connections, authority grants, and link bindings. These maxima are
admission rules: oversized work returns `PlannerLimitExceeded` before planning.
There is no fallback or delegation endpoint in the offer.

The standard reference host advertises `conduit.planner/full@1`. The actual
browser/WASM host advertises `conduit.planner/browser-wasm@1` and invokes the
same deterministic contract inside its own WASM instance before lowering and
executing its fragment. The browser path does not call a std process to plan.
Both current profiles have allocating scratch state before Play start. Neither
claims an allocator-free bounded embedded implementation.

The planner host advertisement is consulted only to select a truthful profile
and admit the request. Host ID, boot ID, offer generation, and profile ID of the
planner are not passed into plan construction and therefore do not enter
`PlanId`, `FragmentId`, placements, or validity. Equivalent portable inputs
produce the same plan regardless of which capable host invokes the contract.

A target host does not need a planner offer. Its ordinary advertisements remain
complete planning input, and its exact fragment retains the same validation,
lowering, execution, and Sign contracts. In particular, the Pico profile
continues to advertise no planner capability while accepting the generated
bounded fragment used by the existing firmware path.

## Bounded embedded eligibility

The portable core offer and limits compile in `conduit-core` without `std`. A
future constrained implementation may use fixed scratch storage and a
restricted deterministic search, advertise smaller limits, and return the same
structured limit refusal. #468 does not claim that the Pico W runs the general
planner; it establishes that the host contract does not forbid such a profile.

## Proof boundary

- `portable_capability` compares full and browser-profile results produced by
  planner hosts with different host/boot identities.
- The bounded-profile negative proves explicit pre-planning refusal.
- The non-planner negative proves capability invocation cannot be inferred from
  target eligibility.
- The browser-runtime tests and WASM build exercise local browser planning as
  part of the existing browser start path.
- Existing Pico lowering, firmware, and verifier suites remain the proof that a
  non-planner constrained target can consume and execute its exact fragment.

WASM compilation is not by itself browser execution, and a Thumb build is not
physical Sign. This capability boundary changes neither proof class.
