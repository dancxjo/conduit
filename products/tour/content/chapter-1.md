---
page: form-basics
route: one-program-many-computers
companion: form-laboratory
stage: canonical-form:meet-one-gear|run
stage: canonical-form:edit-one-gear|run
stage: canonical-form:branch-a-cord|run
---
# One Program, Many Computers

Conduit lets you make **one logical computer — a Body — from one or many physical or virtual computers**. One Body might be a single multicore machine; another might combine a browser, laptop, VM, and microcontroller. The point of this chapter is smaller: build one Form you can run and read.

## Gear, Port, Cord, Form

A **Form** is a program made from connected **Gears**. Each Gear has typed directional **Ports**, and each **Cord** names one exact connection between an output Port and an input Port.

Start with one tiny Form:

```conduit run
form meet-one-gear {
    words: text/literal("hello")
    change: text/upper
    result: presentation/text

    words > change > result
}
```

Run it, then inspect the graph. The source and the Patchbay show the same Form from different views. The Patchbay **projects** checked Form truth; it is not the Form itself.

## Edit one Gear without rewriting its neighbors

Because the surrounding Cords and Ports stay compatible, you can change one Gear and keep the rest of the Form intact.

```conduit run
form edit-one-gear {
    words: text/literal("make this loud")
    change: text/upper
    result: presentation/text

    words > change > result
}
```

## Branch one output explicitly

Fan-out is explicit: one output Port can feed multiple downstream inputs when each Cord is named.

```conduit run
form branch-a-cord {
    source: text/literal("sos")
    loud: text/upper
    show: presentation/text
    morse: text/morse(80)
    light: presentation/indicator

    source > loud > show
    source > morse > light
}
```

If you try an incompatible connection, the refusal is local and typed: this Form fails admission before Play, and nearby Forms are unaffected.
