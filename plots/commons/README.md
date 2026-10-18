# Commons places

Run `cargo xtask check plots run --deterministic --plot commons` to check and
execute the actual Source through an installed std Plan and kernel Play.

Commons owns three exact named places: `commons/hub`, `commons/workshop` and
`commons/creche`. The typed `CommonsPlaces` Fore carries their names and open
semantic role `commons/place`, two containment relations and two directed
reachability relations from Hub. These are authored paths inside this finite
domain; they do not grant permission or assert a live transport Line.

The checked contract fixes all endpoints, role/relation meanings and relation
cardinalities. Names are bounded to 32 UTF-8 bytes. The 8 KiB runtime boundary
holds the bounded canonical specimen; the oracle checks exact emitted values,
Source/checked/expanded/Plan/Play identities and unchanged allocation capacity.
Unknown/duplicate places, self or wrong-meaning relations, missing/excess
relations, unknown roles and oversized names refuse before Play. Malformed
Boolean runtime input refuses without emitting a graph.

The domain Types live in this Source, with no global Room enum or renderer
geometry. The finite current-owner encounter is described below; deterministic
linear/cross-Mask traversal remains #5611. This proves the #5609 Source contract and deterministic std
execution; it does not complete parent #5359's lifecycle, UI or release gates.
Commons is excluded from Workspace discovery and the initial Body until those
product integrations are ready.


## Current owner encounter (#5610)

`commons/encounter` accepts `CommonsEncounterRequest` and emits
`CommonsEncounter`. Three scalar Source guards (`commons/refuse-invalid-places`,
`commons/refuse-invalid-owner`, `commons/refuse-invalid-navigation`) check the
same request and return a typed Boolean on their named `refused` Fore. These
refusal results carry no action authority. The encounter consumes the checked `CommonsPlaces` value, an
owner-supplied observation, current/target place and a Back flag. Its checked
whole-value law admits staying, entering either authored Hub destination, or
returning from an entered context to Hub. Back does not add reverse reachability.
The output preserves the exact graph and observation, names the selected place,
and states whether that context permits requesting owner actions. This policy
can withhold an action; it never grants one.

The observation distinguishes absent, lulled, awake, fulfilled and unknown.
Absent/unknown cannot carry a Body identity; present lifecycle observations must
carry one. Body identity is bounded to 256 bytes, lifecycle to 16 bytes and
revision to U64. The owner adapter constructs this value from the actual retained
session immediately before preparation. A Mask cannot submit a replacement
observation or action catalogue.

The private `products/conduit/src/body_owner/commons_encounter.rs` assembly
projects the Source output alongside the actual current owner Face. It retains
only selected context, a checked evaluator and the current software Mask
execution/acknowledged Show. It owns no Body state. Place roles, names and both
relationship meanings come from the evaluated Source. Existing owner action
identities, intents, targets and argument contracts are preserved. Missing
inspection remains explicitly unavailable. Crèche withholds Body operations.

| Source meaning | Current observation | Face / checked return |
|---|---|---|
| Hub reaches Workshop or Crèche | Current owner or no owner | Checked context navigation; no lifecycle effect |
| Back from an entered context | Same retained Body/session | New context generation and Face; no reverse graph edge |
| Context permits Body requests | Current lulled clock owner and exact Host offers can plan its current checked resident | Existing typed clock interval action; actual Mask interaction Fore then the existing owner workset replacement |
| Context withholds requests, owner awake/fulfilled, missing offers or missing inspection | Current owner truth, never a Mask boolean | Explicit unavailability; no accepted effect |

This first owner vertical supports the existing checked clock interval change.
Other return routes, including generic Wake/Lull, remain unavailable here until
their actual owner route is supplied. It does not infer admission from a generic
BodyFace action or a Patchbay label. The existing ordinary clock entrance keeps
its own Wake/Start/Lull behavior. No new lifecycle dispatcher is introduced.

The retained authenticated Part observation must match the current Host, Boot
and offer generation; a missing or stale observation refuses explicitly.
Current owner Face, complete Host advertisement, context generation and actual
acknowledged Show are rechecked before returning an action. The current request
must exactly match the current owner's original action contract. The software
terminal execution performs the ordinary typed interaction Fore before the
single existing owner effect. One request consumes pending Show custody;
navigation, failed output and cancellation retire it. Unknown/wrong-direction
navigation, stale Face/context/Show, changed Body/Host/offers, unsupported action,
wrong target or typed argument refuse before owner mutation.

Every request Fore explicitly admit at most 8 KiB. The encounter program has a
conservative 27,326-byte output bound; runtime queues admit 32 KiB and one item.
Preparation checks those bounds for the three refusal guards and encounter program. The
unchanged C1 closed graph retains its separate 8 KiB execution boundary. Its prepared evaluator retains fixed
output storage during evaluation; installed std tests also compare kernel value
allocation capacities. Structured request encoding and Face assembly allocate
only during projection preparation, outside Play. Native value contracts and
whole-value invariants are checked explicitly there; a structural frame accepted
by the generic pure evaluator is not itself a validated current observation. No growing storage is hidden
inside a running Commons Play.

`cargo xtask check plots run --deterministic --plot commons` remains the reviewed
C1 entrance. `cargo xtask check plot-s3`,
`cargo xtask check workspace-test-hosts-std` and
`cargo xtask check workspace-test-products` include the Source, installed std
Plan/Play and actual owner regressions respectively. Focused internal tests are
`conduit-plot --test commons_places`, `conduit-std-host --test commons_places`
and the `conduit --bin conduit commons_encounter` filter. The std oracle records actual
Source/checked/expanded/Plan/Play/Host/Boot and kernel Sign evidence; the owner
trace records original Body identity, acknowledged Show, typed interaction,
retained workset change, refreshed Face and exactly one accepted effect.

This is checked-source, production-kernel std and software owner/Mask fixture
proof. Commons remains excluded from initial Body/Workspace discovery. Full
linear traversal and independent cross-Mask reconstruction remain #5611; native
shell integration belongs to #5356 and graphical/native walking to #5360.
These fixtures do not establish QEMU scanout, Chromium, physical input, a
published journey, parent #5359 completion or stable release acceptance.
