---
page: form-basics
route: one-program-many-computers
companion: form-laboratory
stage: canonical-form:meet-one-gear|run
stage: canonical-form:edit-one-gear|run
stage: canonical-form:branch-a-cord|run
---
# One Program, Many Computers

Conduit lets you make **one logical computer — a body — from one or many physical or virtual computers**. One body might be a single multicore machine; another might combine a browser, laptop, VM, and microcontroller. The point of this chapter is smaller: build one form you can run and read.

## gear, port, cord, form

A **form** is a program made from connected **gears**. Each gear has typed directional **ports**, and each **cord** names one exact connection between an output port and an input port.

Start with one tiny form:

```conduit run
form meet-one-gear {
    .    words: text/literal("hello")
    change: text/upper
    result: presentation/text

    words > change > result
}
```

Run it, then inspect the graph. The source and the Patchbay show the same form from different views. The Patchbay **projects** checked form truth; it is not the form itself.

## Edit one gear without rewriting its neighbors

Because the surrounding cords and ports stay compatible, you can change one gear and keep the rest of the form intact.

```conduit run
form edit-one-gear {
    words: text/literal("make this loud")
    change: text/upper
    result: presentation/text

    words > change > result
}
```

## Branch one output explicitly

Fan-out is explicit: one output port can feed multiple downstream inputs when each cord is named.

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

If you try an incompatible connection, the refusal is local and typed: this form fails admission before play, and nearby forms are unaffected.
