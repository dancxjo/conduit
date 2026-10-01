## Normal close and abnormal terminal projections

For closing flow:

```conduit
items >> consume
items| >> finish
items! >> explain
```

Law:

```text
items     ordinary values
items|    meaningful normal close; legal only for T...|
items!    abnormal terminal truth
```

Silence/quiescence is neither `|` nor `!`.

`endpoint!` is not exception throwing. It exposes typed semantic terminal truth as an ordinary graph track.

Successful internal fallback/replan does **not** manufacture a semantic `!` if the endpoint continues satisfying its contract.

No `try`, `catch`, `throw`, hidden unwinding, or global error bus.

Provenance: #3970, #3999, #4001.

---

## Terminal transduction and default abnormal propagation

gears/forms transform **values plus terminal truth**.

Reusable terminal laws must distinguish at least:

- modality-preserving per-value transform;
- filter;
- reducer/collector;
- buffered/framing transform;
- domain-specific terminal behavior.

A closing-flow consumer whose fore declares `T...|` receives normal-close semantics as part of that port contract. Authors should not need to wire `items|` merely so a collector knows its input ended.

Normal close may trigger finite flushing before downstream close when the exact contract says so. Abnormal terminal must **not** masquerade as normal-close flushing unless a reviewed fault-finalization law explicitly permits it.

Do not universally copy the last payload into close. Retain last-value context explicitly (for example through keep/`T?`) only where needed.

Default abnormal composition rule:

> **An unhandled abnormal terminal in a child gear/form propagates as abnormal terminal truth of the containing form.**

Catching/routing `x!` is not by itself successful recovery. The containing form must still satisfy its checked obligations. If recovery faults, the unrecovered terminal propagates.

Keep three layers distinct:

~~~text
mechanism/Back trouble
recovery/fallback/replan activity
semantic endpoint abnormal termination
~~~

Successful internal recovery remains evidence and must not manufacture semantic `!`.

Provenance: #3999, #4000, #4047.

---

## Semantic cancellation: `~`

Preferred/canonical control projection:

```conduit
deadline >> work~
work! >> explain
```

`work~` is an explicit semantic cancellation request **only when that gear's fore declares such a cancellable control surface**.

It is distinct from:

- observed abnormal terminal `work!`;
- typed `cancelled` terminal disposition;
- scheduler/lifecycle cancellation.

Sending cancellation does not prove it succeeded.

Provenance: #4049.

---

## Multi-input temporal relationships must be explicit

A pure expression never secretly synchronizes two independent runtime inputs.

Distinct operations remain distinct:

- zip next A with next B;
- combine latest;
- sample a current value when another event arrives;
- keyed join;
- merge arrivals;
- race/first result.

These are ordinary semantic gears/forms with exact finite pending-state, pressure, and terminal laws.

No hidden `combineLatest`, timestamp-proximity join, or source-order synchronization.

## Fan-in

Independent producers may not silently write the same ordinary single-producer input.

Merge/race/join must be explicit.

The comma-list merge sketch is rejected. Canonical concise merge uses the standard `><` gear glyph from #4335, e.g. `a >< b >> merged`; checked expansion still contains one ordinary `flow/merge` gear.

## Arbitration

Ambiguous races refuse at checking unless exclusivity is proven by routing or an explicit arbitration kind owns the competition.

No “first thread wins,” source-order priority, or hidden last-writer-wins.

Provenance: #4046, #4050, #4064.

---

## cord delivery, fan-out and pressure

Ordinary fan-out remains boring source:

~~~conduit
temperature >> display
temperature >> history
temperature >> safety
~~~

Its behavior belongs to checked cord/port contracts, not source order or a hidden dispatcher.

Relevant contracts must define:

- coupled vs independently pressured delivery;
- finite pending capacity/resource requirements;
- whether every accepted value remains owed;
- explicit sampling/coalescing/drop policy where coverage changes;
- cancellation/provider-loss interaction;
- close/fault propagation per branch;
- evidence for pressure/supersession/drop when semantically relevant.

Default direction: ordinary fan-out preserves delivery promises. A slow branch must not silently make another branch lossy.

Authors should not spell queue byte arithmetic when a reviewed finite profile/default can supply it safely.

Fan-out is legal only when the info/resource ownership contract permits duplication/shared reference. Capability/resource-valued values must not become forgeably cloneable.

Independent fan-in remains explicit under section 16: merge, race, zip, combine-latest, sample and joins are distinct meanings.

Provenance: #4045, #4046.

---
