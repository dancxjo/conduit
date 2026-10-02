---
page: plot-basics
journey: conduit-tour
route: one-program-many-computers
companion: plot-laboratory
stage: canonical-plot:meet-one-gear|run
stage: canonical-plot:edit-one-gear|run
stage: canonical-plot:branch-a-cord|run
---
# One Program, Many Computers

Conduit lets you make **one logical computer — a body — from one or many physical or virtual computers**. One body might be a single multicore machine; another might combine a browser, laptop, VM, and microcontroller. The point of this chapter is smaller: build one plot you can run and read.

## gear, port, cord, plot

A **plot** is a program made from connected **gears**. Each gear has typed directional **ports**, and each **cord** names one exact connection between an output port and an input port.

Start with one tiny plot:

```conduit run
plot meet-one-gear {
    words: text/literal("hello")
    change: text/upper
    result: presentation/text

    words >> change >> result
}.
```

Run it, then inspect the graph. The source and the Patchbay show the same plot from different views. The Patchbay **projects** checked plot truth; it is not the plot itself.

## Edit one gear without rewriting its neighbors

Because the surrounding cords and ports stay compatible, you can change one gear and keep the rest of the plot intact.

```conduit run
plot edit-one-gear {
    words: text/literal("make this loud")
    change: text/upper
    result: presentation/text

    words >> change >> result
}
```

## Branch one output explicitly

Fan-out is explicit: one output port can feed multiple downstream inputs when each cord is named.

```conduit run
plot branch-a-cord {
    source: text/literal("sos")
    loud: text/upper
    show: presentation/text
    morse: text/morse(80)
    light: presentation/indicator

    source >> loud >> show
    source >> morse >> light
}
```

If you try an incompatible connection, the refusal is local and typed: this plot fails admission before play, and nearby plots are unaffected.
