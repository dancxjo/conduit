# Step 6 — Same Face, different implementation

A Gear's Face fixes its meaning; a Host may satisfy that Face directly or the planner may recursively open its reviewed Form Back.

```conduit compare
form same-morse-caller {
    message: text/literal("HELLO")
    morse: text/morse(40)
    light: presentation/indicator

    message > morse > light
}
```

The direct leaf uses the browser Host's native `text/morse` implementation. A Host without that direct offer can instead open the `text/morse` Back into character decomposition, Morse lookup, interspersed gaps, flattening, and timing, then continue opening reviewed Backs until every remaining Gear has a leaf implementation it actually offers.

Both realizations begin with the same `text/morse` Face and produce the same canonical Morse pattern. The caller does not change, and it does not need to know which implementation was selected. Source and checked Form identities agree; expanded Form, Plan, and implementation identities remain honestly different.

This compositional implementation of the useful vocabulary is what lets Conduitos remain a minimal viable Conduit Host. It can begin with a small set of primitive leaf capabilities and still support much richer behavior by recursively composing reviewed Forms instead of implementing every useful Gear as an operating-system primitive.

The side-by-side controls deliberately expose the architecture for this lesson. They are a teaching aid, not ordinary execution modes that authors choose throughout the Tour.
