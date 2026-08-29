# Gear Lab

The same Run button accepts other small, finite Forms. These examples are ordinary Conduit source; the palette and execution choices come from this browser Host's real offers.

## One meaning, two realizations

This listing still says only `text/morse`. Use the realization control to run the same checked Face as one optimized leaf or open its reviewed Back into characters, lookup, gaps, flattening, and timing.

```conduit run recursive
form recursive-morse {
    message: text/literal("HELLO")
    morse: text/morse(80)
    light: presentation/indicator
    message > morse > light
}
```

## Exact scalar math

Scalar values carry six exact decimal places. Scaling is checked and the result is manifested only by the planned presenter.

```conduit run
form math-lab {
    source: scalar/literal(1.5)
    scale: math/scale(2.0)
    result: presentation/scalar
    source > scale > result
}
```

## Typed logic

Boolean logic has no truthiness coercion: one canonical Boolean goes in and one canonical Boolean comes out.

```conduit run
form logic-lab {
    source: boolean/literal(true)
    invert: logic/not
    result: presentation/bool-value
    source > invert > result
}
```

## Fan out, then decide

One scalar fans out atomically to two transforms and reconverges at an exact two-input comparison. There is no topology rule in the book.

```conduit run
form fanout-lab {
    source: scalar/literal(0.5)
    scaled: math/scale(2.0)
    quiet: math/deadband(0.6)
    compare: logic/compare("gt")
    result: presentation/bool-value

    source > scaled > compare.left
    source > quiet > compare.right
    compare.out > result
}
```

## Structured language

The tokenizer and annotator exchange bounded canonical structured Info, not JSON-shaped convenience data in the page.

```conduit run
form language-lab {
    tokens: language/tokenize-four("Bright stars shine.")
    annotate: language/annotate-four
    result: presentation/structured-info
    tokens > annotate > result
}
```

Try changing a Kind to one marked unavailable in the palette. The checker or planner will refuse before Play and report which boundary was missing.
