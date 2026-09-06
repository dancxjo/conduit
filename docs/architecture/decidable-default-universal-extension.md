# Decidable by default, universal by explicit extension

**Status:** durable architecture direction; executable proof is tracked separately.  
**Related:** #2682, #2685, #2686, #2687, #2688, #2689, #2690, #2691

Conduit should not make every Form pay the theoretical cost of computational universality.

The default Form calculus remains deliberately restricted enough to preserve strong static reasoning. Universal computation exists as an explicit escalation for Forms or sub-Forms that truly require it.

The governing laws are:

> **Decidable by default. Universal by explicit opt-out.**

> **Universality is available, not ambient and not contagious.**

> **Forms may continue indefinitely without becoming universal.**

> **Every Plan and Play is a finite, typed, resource-admitted embodiment, including Plays that realize universal computation.**

> **An implementation must not possess materially more authority than the exact admitted realization it is executing.**

These are architectural commitments, not claims that current Conduit already proves universality, exhaustive analysis, or hostile-code confinement.

## 1. Why keep the default decidable

A purely finite or otherwise restricted calculus can preserve facts that a universal language cannot promise in general. For qualifying Forms, Conduit should be able to retain exact finite transition semantics, reason about reachability and invariants, prove termination where the semantics support it, and admit deadline-bounded regions when the selected implementations have compatible finite worst-case bounds.

Once unrestricted universal computation is ambient, general halting becomes undecidable. General behavioral equivalence and exhaustive future-state reasoning likewise lose their global finite-state character. A single fixed capacity cannot always be proved sufficient for all future execution, and arbitrary universal work cannot carry an automatically derived worst-case execution time.

Those are real losses. Ordinary Forms should not inherit them merely because Conduit is capable of expressing a universal machine somewhere else.

The default language therefore rewards restraint with stronger knowledge.

## 2. The default calculus

A default Form should remain in the strongest analysis class its checked semantics justify. The exact representation may become a property lattice rather than a single enum, but useful classes include:

```text
acyclic / combinational
    finite graph, no retained recurrence

finite-state recurrent
    explicit State, finite state space, exact transition semantics

bounded terminating
    semantic completion is proved within a finite bound

reactive continuous finite-state
    indefinite lifetime driven by the environment,
    finite internal state and bounded work per admitted step/event

universal
    explicit use of semantics whose abstract capacity has no fixed global finite ceiling
```

Deadline/WCET properties may overlay the first several classes when a region and all of its dependencies have compatible proved worst-case bounds.

The checker should preserve the strongest facts it can prove. It should also explain why a stronger class was lost.

## 3. Continuous does not mean universal

A thermostat, compositor, server, sensor pipeline, user interface, audio graph, or robot controller may remain active for months or years while still being finite-state.

Its lifetime is open-ended because new observations keep arriving, not because its internal abstract memory is unbounded.

Therefore:

```text
indefinite lifetime
!= universal computation
```

A continuous finite-state Form must not be forced to opt out of the decidable default merely because it has no predetermined final transition.

Continuous execution should preserve the distinction between semantic lifetime and concrete realization lifetime. A Form can remain the same continuant while Plans and Plays are replaced as Hosts, Boots, resources, Lines, or admitted capacities change.

## 4. The universal boundary is explicit

A Form crosses into universal computation only through semantics that deliberately break the globally finite abstract state bound, such as a reviewed extensible natural-number, tape, memory, or equivalent basis.

The exact source spelling is intentionally unresolved. The important property is that the boundary cannot be crossed accidentally.

A suitable design might use an explicit Form-level declaration, a semantic requirement inferred from universal primitives and acknowledged by the author, an explicitly universal nested Form, or another capability/effect-style mechanism.

A decorative `universal = true` flag is insufficient if the checker cannot explain what caused the escalation. The checked semantics should identify the construct or dependency that made the stronger guarantee unavailable.

A universal sub-Form should degrade analysis only along the dependencies that actually depend on it. Unrelated finite regions remain finite.

## 5. Universality stays Conduit-shaped

The universal extension must not become a hidden second programming language.

State remains explicit and typed. Recurrence crosses State/delay boundaries. Same-generation ordinary dataflow remains acyclic. Conditional control remains ordinary typed comparison, Boolean information, selection, and next-state composition.

Universality does not require ambient mutable variables, assignment statements, `while`, `for`, `goto`, `break`, `continue`, truthiness, arbitrary callbacks, hidden dynamic graph mutation, a universal `Any`, or an ambient heap.

Nor is it sufficient to add one opaque `compute/wasm`, `compute/python`, or `compute/universal-machine` gear and declare the language universal because that gear secretly contains a general computer.

The architectural witness should be compositional. #2682 uses a two-counter Minsky machine or an equivalently small universal model to prove that ordinary reviewed Conduit semantics can express arbitrary computation inside the explicitly universal class.

## 6. Unbounded meaning, bounded embodiment

Even an explicitly universal Form does not receive infinite physical resources.

Every Plan and Play remains finite and exact. A realization may admit finite limits such as:

```text
state capacity
working-value bytes
queue depth and buffered bytes
fuel / transition work
host-operation concurrency
Line and Resource capacity
mandatory Sign storage
timing/resource reservations where relevant
```

The planner need not prove that an arbitrary universal computation will eventually halt. It must prove that this embodiment cannot consume more than its admitted physical resources and authorities.

This changes the strongest global promise from:

> this computation will always fit

into:

> this computation cannot exceed what this Play was given

For default finite Forms, stronger sufficiency proofs may still be possible and should be preserved.

## 7. Termination is not exhaustion

Universal and continuous computation require more precise lifecycle semantics.

Keep distinct:

```text
HALT / semantic completion
quiescent / awaiting input
LULL / Body-level suspension
cancelled
fuel exhausted
State capacity exhausted
other Resource capacity exhausted
failed
Host / Boot / Resource / Line lost
Plan retired or replaced
continued operation
```

A Play reaching its fuel or memory bound did not thereby prove that the Form halted. A replan is not a semantic restart. Quiescence is not completion. Lull is not failure.

#2690 owns the machine-readable result vocabulary and proof.

## 8. Replanning preserves meaning only when continuity is explicit

A long-lived Form may need a replacement realization because capacity changes, a Host reboots, a Line disappears, policy changes, or a better implementation becomes available.

That changes Plan and Play identity. It does not automatically change checked Form identity.

State continuity across such a replacement must be typed, bounded, and explicitly admitted. If continuity cannot be preserved, Conduit must report reset/loss/refusal rather than silently resetting State and presenting the result as continuation.

Old Boot authority, resource bindings, and initialized implementation truth must not leak into the replacement realization.

#2691 owns this proof.

## 9. Real-time regions remain stricter

A universal outer language must not dissolve hard-real-time meaning.

A deadline/WCET-admitted region may depend only on work whose exact selected realization has a compatible proved finite worst-case bound. Arbitrary universal work may exist elsewhere in the Form without invalidating an isolated deadline region, but it may not enter that region through an unknown-cost dependency.

#2689 owns this boundary.

## 10. Security consequence: expressiveness and authority are orthogonal

Computational universality must grant no additional effect authority.

A universal calculator with no admitted network operation still has no network authority. A finite presentation Gear does not gain filesystem access merely because the containing native process happens to have it.

Conduit already separates availability, selection, authority, and execution semantically. The intended stronger property is:

> **A computation has no path to an external effect for which its admitted realization lacks authority.**

This is potentially safer than an ambient process model because a computation may be arbitrarily clever while physically lacking access to the filesystem, network, microphone, camera, actuator, credential, or other capability it was never given.

But planner metadata alone does not prove hostile-code isolation. A native implementation that can bypass Conduit and directly invoke broadly privileged OS APIs defeats the confinement story even if the Plan is perfectly described.

#2685 therefore distinguishes semantic least-authority modeling from mechanism-level confinement. Different Hosts may establish different proof strengths. A cooperative std process, a capability-shaped WASM import boundary, a future ConduitOS kernel boundary, and a remote authenticated Host do not automatically have the same isolation properties.

Do not claim generic `secure=true`. State the exact enforcement/trust class proved.

## 11. What we deliberately keep

Adding explicit universality does not revoke the existing Conduit disciplines:

- exact typed Ports and Cords;
- explicit State;
- acyclic ordinary same-generation dataflow;
- finite Plans and Plays;
- bounded queues, buffers, resources, and host-operation concurrency;
- explicit pressure and exhaustion;
- exact Host/Boot/implementation/Resource/Line identity;
- availability distinct from authority;
- planning distinct from authority issuance;
- one execution kernel;
- honest proof classes;
- inspectable decisions and effects.

The universal extension must fit inside those rules rather than becoming an exception to them.

## 12. Author and operator experience

The common case should require no theoretical ceremony. A normal Form remains in the default class and receives the strongest available analysis automatically.

When a Form crosses the universal boundary, the tooling should say why. Patchbay should eventually be able to explain statements such as:

```text
This Form is finite-state because all retained State has finite domains.

This Form is continuous but remains finite-state because continuation is externally driven and internal State is finite.

This sub-Form is universal because it uses an extensible memory primitive.

Termination cannot be proved for this dependency after that boundary.

This Play has K fuel. K is an embodiment limit, not a semantic halt bound.

This deadline region excludes the universal dependency because no compatible WCET proof exists.
```

Universality should feel like intentionally opening a larger computational door, not like discovering after the fact that an innocent Form lost all of its useful static guarantees.

## 13. The resulting theoretical object

With these constraints, Conduit is aiming at something like a:

> **decidable-by-default, typed reactive dataflow calculus with explicit State, an explicit computationally universal extension, finite resource-admitted embodiments, and capability-oriented effects.**

The concise project maxim is:

> **Decidable by default. Universal when needed. Unbounded meaning, bounded embodiment, explicit authority.**

That is the bargain the architecture should preserve.