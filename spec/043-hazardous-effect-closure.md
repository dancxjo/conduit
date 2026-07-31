# Hazardous effect closure current

Status: normative seed for issue #95.

## Purpose and claim boundary

An individually narrow effect can become materially stronger when combined with
other effects. `HazardClosurePolicy` lets a domain describe combinations that
must be denied or independently permitted before a plan starts. The analysis is
over declared effects and exact bindings. It is not source-code intent
analysis, a natural-language classifier, or proof that unclassified behavior is
safe.

Core owns no list of hazardous actions. Effect classes, transfer meanings,
toxic-combination rules, resources, audiences, realms, budgets, and permit
operations are versioned domain-owned descriptors. The same core machinery can
therefore express a domain policy without adding vendor, device, application,
or morality concepts to `conduit-core`.

This contract separates:

- ordinary `EffectRequirement` declarations and resolved `PlanAuthority`
  bindings;
- domain policy and exact transfer facts;
- the bounded closure decision and secret-safe proof tree;
- independently approved exceptional permits;
- execution-plan persistence;
- host observations and run evidence.

An accepted closure decision says only that the supplied, complete exact plan
facts do not match a policy rule without a valid permit.

## Effect classes and ordinary authority

**HZD-001.** An `EffectClassBinding` pins one domain descriptor and the
identical ordinary `AuthorityConstraintRef`. An effect belongs to a class only
when its resolved effect requirement carries that exact constraint. Names
without matching semantic hashes never classify an effect.

**HZD-002.** Class traits for persistence, delegation, distribution, and
administration are policy inputs, not inferred core facts. A domain must mint a
new descriptor identity when its class meaning changes.

**HZD-003.** Every analyzed effect is a resolved `PlanAuthority`. Its effect,
grant, node, action, exact resource selector, audience, host, constraint set,
administrative subject, and policy-budget bindings participate in the closure
subject. Analysis does not replace ordinary resolution.

The compiler passes the complete lowered plan authority collection into the
analyzer. Consequently primitive effects and effects exposed through
composites or accepted satisfaction are visible when lowering has represented
them as ordinary plan authorities. Host services, administrative providers,
distributed endpoints, and delegated work have the same obligation: every
operation that may produce an effect must be declared and resolved as an
ordinary effect. An undeclared host behavior is a contract violation outside
what this analyzer can prove.

## Policy-owned toxic combinations

**HZD-004.** A `ToxicEffectPattern` matches an exact effect class and may
further constrain exact resource, audience, host, realm, or policy-budget
descriptor. It may also require or exclude the four class traits. Omitted
selectors are wildcards owned by that explicit rule, not inferred equivalence.

**HZD-005.** A `ToxicCombinationRule` is satisfied only by distinct effects
matching every pattern and every declared flow. Rule, pattern, class, and flow
ordering is canonical and deterministic. Discovery, source, registry, and
scheduler order do not change the result.

**HZD-006.** `EffectFlowBinding` records an exact declared transfer from one
effect to another. `ToxicFlowRequirement` refers to pattern indexes and a
pinned domain-owned transfer descriptor. Transfer can mean an output,
credential, artifact, host observation, administrative subject, or other
domain fact; core does not guess data flow from names.

This makes multi-stage discovery, enrollment, installation, execution, and
redelegation visible when the plan or resolver declares the corresponding
exact links. Missing declared links do not permit core to invent speculative
ones.

**HZD-007.** Different hosts, realms, processes, audiences, resources, epochs,
or delegation paths are never an ambient safety boundary. They change exact
matching only when the policy explicitly constrains those facts.

**HZD-008.** Removing an optional graph feature requires analysis of the newly
resolved complete closure. Degradation cannot carry forward an earlier decision
or omit effects that remain reachable.

## Live transition closure

**HZD-009.** Before a live replacement begins, transition analysis combines
the old generation with every new-generation and rollback-reserve authority
that can coexist, plus their respective exact flows. Each generation may be
safe alone while the overlap is denied.

`analyze_transition_effect_closure` defines that combined semantic check now.
Issue #57 remains responsible for invoking it at the transition boundary and
for representing the actual overlap and rollback reserve completely. This
specification does not claim that the current repository contains the full
live-transition controller.

The transition subject is distinct from a standalone plan subject and binds
both authority collections, both flow collections, the target epoch, and the
monotonic time basis. A standalone permit cannot be replayed for overlap.

## Exact exceptional permits

**HZD-010.** A toxic match denies before start unless one `HazardPermit`
matches the exact policy, rule, closure subject, epoch, combination scope, and
monotonic time basis and is currently valid.

The combination scope hashes the rule and the exact selected effects, including
their resolved plan facts. A permit for another plan, epoch, host, realm,
resource, audience, artifact-bearing effect, budget-bearing effect, or selected
combination therefore does not authorize the current closure.
Every distinct matched scope requires its own permit. Authorizing one occurrence
of a rule does not suppress another occurrence elsewhere in the same closure.

**HZD-011.** Every permit carries the administrative containment proof defined
by specification 041. Its proposal uses the policy permit class and exact
permit-operation descriptor. The proposal subject binds plan, epoch, and the
combination scope through its budget field. Approval, commit, and execution
identities remain distinct and the requesting plan cannot approve its own
exception.

**HZD-012.** Permit validity is finite. `not_before_tick` is inclusive,
`expires_at_tick` is exclusive, and all permit and approval times use the exact
closure time basis. Revalidation at current time rejects expired approval or
permit state.

Permit persistence in a plan is not run evidence. A host records the decision,
permit use, and effect execution as distinct evidence.

## Bounded canonical analysis

**HZD-013.** Policy limits bound effects, classes, rules, patterns per rule,
flows, permits, proof nodes, and search steps. Every count and search increment
uses checked arithmetic. Exceeding a limit is deterministic denial before any
node or transition begins.

**HZD-014.** The reference analyzer uses caller-provided fixed proof storage
and no allocator. It visits rules and candidate authorities in canonical
semantic order and emits only validated descriptor and effect identifiers.
Arbitrary resource values, secret handles, credentials, and hostile input are
not copied into proof nodes.

**HZD-015.** A proof tree names the exact policy, rule, selected effects, and
permit decision needed to explain the minimal matched combination. Proof
storage exhaustion fails closed; it never truncates an accepting explanation.

`conduit-core` remains `#![no_std]` and allocator-free. Hosted serialization,
larger indexes, durable policy storage, and presentation remain above core.

## Exact plan and compiler boundary

**HZD-016.** Execution-current plan schema stores an optional `PlanHazardClosure`
containing the epoch, exact closure subject, policy, flow bindings, permits, and
decision identity. Hosted governed plans use
`conduit.execution-plan`. Earlier current plan through current plan identities remain
readable and unchanged.

The compiler seals descriptor, rule, policy, approval, permit, closure-subject,
and decision identities. It runs closure analysis before returning a runnable
plan. Portable plan validation repeats policy and permit validation and repeats
the decision at the current monotonic observation. Mutation, expiry, or a
different complete authority set fails validation.

`conduct --check` and `conduct --explain` return exit status 2 and the stable
hazard diagnostic before execution. Presentation is derived from the
diagnostic; it is not the enforcement decision.

## Diagnostics and conformance

**HZD-017.** Stable diagnostics are:

- `CND-HZD-001` unsupported policy version;
- `CND-HZD-002` invalid descriptor;
- `CND-HZD-003` identity mismatch;
- `CND-HZD-004` effect limit exceeded;
- `CND-HZD-005` invalid flow;
- `CND-HZD-006` invalid rule;
- `CND-HZD-007` search limit exceeded;
- `CND-HZD-008` proof storage exceeded;
- `CND-HZD-009` toxic combination;
- `CND-HZD-010` exact permit missing;
- `CND-HZD-011` permit scope mismatch;
- `CND-HZD-012` permit expired;
- `CND-HZD-013` permit approval invalid;
- `CND-HZD-014` transition subject invalid.

**HZD-018.** `conformance/c2/hazard-closure.json` contains 22 independently
dispatched cases. The reference test reconstructs and executes each isolated
control, toxic combination, exact permit, plan/epoch/artifact/host/realm/budget
permit mismatch, per-occurrence permit, permit expiry, invalid approval,
propagation, composite, remote/federated, constraint-distinction, overlap,
degradation, and bounded-failure case. Merely recognizing a case identifier is
not conformance.

The fixtures prove deterministic behavior of declared facts. They do not prove
that an external domain taxonomy is complete, that a host reports all effects,
that a distributed provider is honest, or that a physical deployment is safe.
