# Step 0 — Hello, light

A Form describes a flow of meaning: edit the message, press Run, and watch the browser manifest its timed indication.

```conduit run
form hello-light {
    message: text/literal("SOS")
    morse: text/morse(120)
    light: presentation/indicator

    message > morse > light
}
```

The Gears do the work and the Cord carries one exact typed value. The Form names neither a screen nor a physical light.

# Step 1 — Change one Gear

Compatible Gear Faces let one small semantic operation replace another without changing how the surrounding flow is written.

```conduit run
form change-one-gear {
    message: text/literal("hello")
    change: text/upper
    result: presentation/text

    message > change > result
}
```

Change the quoted text and Run again. The same typed Cord now carries ordinary text rather than a timed Morse pattern.

# Step 2 — Fan out explicitly

One emission can enter two compatible branches only because both Cords are present in the Form and admitted atomically by the Plan.

```conduit run
form explicit-fanout {
    source: scalar/literal(0.5)
    left: math/scale(2.0)
    right: math/deadband(0.6)
    decide: logic/compare("gt")
    result: presentation/bool-value

    source > left > decide.left
    source > right > decide.right
    decide.out > result
}
```

Both branches receive the exact same scalar. Nothing is broadcast to an unnamed listener.
