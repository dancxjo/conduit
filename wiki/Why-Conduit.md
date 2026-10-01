Conduit begins with a stubborn observation: a useful program has a **meaning** that is not identical to the machinery presently available to carry it out.

A camera pipeline is not an x86 process graph. A reminder is not a timer file descriptor. A conversation is not a browser DOM. A connection between two pieces of work is not a WebSocket. Those mechanisms may realize the meaning, sometimes very well, but the mechanism is not the meaning.

Conduit tries to keep that distinction explicit all the way from source to execution.

> **Author the intended happening; let planning meet it with the world that actually exists.**

The result is not “write once, run anywhere” in the usual sense. Conduit does not pretend machines are interchangeable. It does the opposite: portable meaning stays portable **because realization is allowed to remain exact, local, finite, and truthful**.

## The central movement: form, plan, play

The most important three nouns describe three different moments.

- A **form** is the authored course of events: what should happen and how semantic work relates.
- A **plan** is one exact, admitted way of realizing that form given the body, hosts, backs, lines, resources, authority, and other current facts.
- A **play** is that plan in motion.

Conceptually:

```text
form
  + current truth about the world
  ↓ planning
plan
  ↓ execution
play
```

This separation is the answer to a surprisingly large family of systems problems.

When a host disappears, the form need not change. When a different implementation is selected, the kind need not change. When a cord moves from local memory to a network carrier, the semantic connection need not change. Conduit changes the **plan** when reality changes rather than quietly rewriting the authored meaning.

That is what “continuous planning” is meant to preserve: not constant recomputation, but the continued availability of planning as the lawful place where changed reality is reconciled with enduring intent.

## Meaning and code

A **type** says what a value means. A **code** declares one exact portable
encoding of that meaning. An info value is one value of the type.

```text
type  = semantic meaning
info  = one typed value
code  = a portable encoding contract
```

A variant such as `work` does not mean the byte `1`; a code can assign it that
tag without making the tag its semantic identity. Current compact codes cover
finite variant tags. See [[Current language surface|Current-language-surface]]
for the checked declaration and its scope.

## Meaning and realization of work

Work has another small cluster of nouns:

- a **kind** says what a reusable operation means;
- its **fore** is the checked function-like signature through which authors call that meaning;
- a **back** is one concrete realization compatible with that kind and fore;
- a **gear** is one configured occurrence of a kind in a form.

A form can therefore ask for semantic work without choosing the final implementation.

```text
kind      what work means
fore      the callable signature presented to the author
back      one way to realize it
gear      one occurrence in the authored form
```

A host may offer several backs for the same kind. Planning chooses among eligible backs using current, finite facts rather than source-code accident.

## ports, cords, and lines

A **port** is a typed directional point declared by a fore and used by gears. A **cord** connects compatible ports. A **line** is concrete carriage that may realize a cord when the connected gears do not share the same local mechanism.

That produces another useful distinction:

```text
cord  = semantic relationship
line  = realized carriage
```

A cord can remain the same cord whether the plan realizes it through shared memory, USB, WebSocket, WebRTC, a relay, or some future mechanism. If networking itself is the meaning, networking appears in the form as semantic work; if networking is merely carriage, it belongs to realization.

## One computer, changing machinery

A **body** is the enduring logical computer.

A **host** is current machinery capable of offering realizations. A **boot** is one current incarnation of a host. A **part** is a durable membership relationship in the body. A body may **wake** into activity and **lull** without ceasing to be the same body.

These distinctions let Conduit say something more precise than “the computer restarted” or “the service moved”:

- same body or a different one?
- same part or a replacement?
- same host with a fresh boot?
- same plan or a replacement plan?
- same play or a new play?

Continuity belongs to the body, not to any one process or machine.

## State is a promise, not a variable

A **keep** says that some current semantic truth must remain available for an exact duration.

That is different from saving data.

- **keep** promises retention of current truth;
- **data** is an independently addressable immutable generation;
- **save** publishes one generation as data;
- **load** recovers typed info from it.

This is why Conduit resists treating every temporal problem as mutation of a heap cell. Duration, publication, identity, and causality are separate promises and should remain visible.

## fore and face are unrelated

The words happen to sound spatial, but they live in different parts of the architecture.

A **fore** is a callable signature, roughly the role a function signature plays in an ordinary language. A **face** is human-facing semantic interaction and presentation. Neither is a specialization or realization of the other.

## Human meaning is not a widget tree

Presentation uses another three-part separation:

- the **face** is the humanly relevant meaning and agency of the encounter;
- a **mask** is one planned realization of that face for a user agent or medium;
- a **show** is one concrete manifested occurrence.

```text
face → mask → show
```

The same face can be shown through a browser, a native surface, speech, terminal text, or another admitted medium without declaring any one of those mechanisms to be the application truth.

A show may even physically remain after its source face is stale. Residue is not current truth.

## Evidence is not authority

A **sign** records bounded evidence about what was selected, attempted, observed, completed, refused, lost, or replaced.

signs are important because distributed and adaptive systems otherwise become stories reconstructed from logs. Conduit instead tries to make important causal facts first-class.

But evidence does not grant permission. A sign may prove that a host exists; it does not authorize that host to perform an effect. Availability, membership, trust, authority, eligibility, selection, and execution remain distinct facts.

## Finite execution, open meaning

Conduit is strict about runtime finitude without requiring every semantic domain to be finite.

A semantic type may describe an open-ended mathematical range. The scheduler still requires actual values, queues, collections, retained state, calls, and other resources to fit admitted finite envelopes.

The rule is:

> **Semantic domains may be unbounded; execution resources may not be.**

This lets the language describe the world honestly without promising infinite machines.

## Quiescence is not completion

A live play may become **quiescent** when no admitted work can presently make progress. Quiescence is a pause, not a terminal state; later admitted work may wake the same play again.

Semantic completion is a stronger claim. When a form explicitly says that structural drain is sufficient completion, the play may terminate normally at that point.

This distinction matters for continuously reactive systems: “nothing to do right now” and “the authored work is finished” are not synonyms.

## The philosophy in one diagram

```text
information                    work
-----------                    ----
type → info                    kind → fore → back
                                  ↓
                                gear
                                  ↓ ports
form ──────────────────────── cords
  +                             ↓
current body/host truth       lines when needed
  ↓
plan
  ↓
play
  ↓
signs

body persists across hosts, boots, plans, and plays
face → mask → show keeps human meaning separate from rendering
keep / data keeps temporal truth separate from publication
```

The vocabulary is intentionally ordinary, but it is best learned in these local clusters rather than as one grand taxonomy.

## What Conduit is resisting

Many systems work by allowing meaning to leak gradually into mechanism:

- deployment files become part of application semantics;
- process identity becomes service identity;
- retries become accidental behavior;
- UI trees become domain state;
- network reachability becomes authority;
- storage layout becomes type meaning;
- mutable memory becomes the only model of time;
- logs become the only source of causal evidence.

Conduit is an attempt to keep those seams visible enough to reason about, check, plan, replace, and explain.

The names and syntax will continue to sharpen, but these separations are the part intended to endure.

For the architectural walk-through, continue with [[Architecture tour|Architecture-tour]]. For exact implemented syntax rather than the conceptual vocabulary, see [[Current language surface|Current-language-surface]].
