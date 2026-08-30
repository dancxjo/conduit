# Same Face, different implementation

A large Host may already have an efficient native implementation of text/morse. A minimal Host should not need to grow an operating-system primitive for every useful high-level Gear merely to understand the same Form.

## Conduit idea

A Gear's Face fixes its meaning. One Host may satisfy that Face with a direct leaf; another realization may recursively open its reviewed Form Back until every remaining Gear has a leaf implementation the available Host actually offers.

```conduit compare
form same-morse-caller {
    message: text/literal("HELLO")
    morse: text/morse(40)
    light: presentation/indicator

    message > morse > light
}
```

## What the run proves

Realization A uses the browser Host's native text/morse leaf. Realization B opens text/morse into character decomposition, Morse lookup, interspersed gaps, flattening, and timing, then plans the primitive leaves. Both begin with the same requested Face and produce the same canonical Morse pattern.

The caller is unchanged because the meaning is unchanged. Source and checked Form identities agree; expanded Form, Plan, and implementation identities remain honestly different. The side-by-side controls expose those identities as a teaching aid, not as author-facing execution preferences.

## Payoff

This is how ConduitOS can remain a minimal viable Conduit Host. It can offer a small reviewed set of primitive leaves while richer vocabulary recursively composes above them. A more capable Host can take an optimized direct leaf. The caller sees the same Face either way and does not need to become a ConduitOS program or a browser program.
