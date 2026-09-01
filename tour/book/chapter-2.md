# Branch a Cord

One value can feed more than one Gear. Draw that choice explicitly and the whole branch stays visible.

```conduit run
form branch-a-cord {
    source: scalar/literal(0.5)
    double: math/scale(2.0)
    quiet: math/deadband(0.6)
    compare: logic/compare("gt")
    result: presentation/bool-value

    source > double > compare.left
    source > quiet > compare.right
    compare.out > result
}
```

Both branches begin at the same output Port and meet again at `compare`.

# Meet the Face

Look at a Gear from the outside and you see its **Face**: its meaning and typed Ports. A caller can connect to that contract without knowing how the Gear is implemented.

```conduit run
form meet-the-face {
    source: scalar/literal(1.5)
    scale: math/scale(2.0)
    result: presentation/scalar

    source > scale > result
}
```

`scale` receives a scalar and emits a scalar. That public shape is enough to compose it here and somewhere entirely different.

# Same Face, different implementation

Some Gears open. `text/morse` has a reviewed Form **Back** made from smaller Gears.

Use **Open Back** on the `morse` faceplate. The original Gear stays put while its checked internal topology opens beneath it. Return to the Face whenever you like; the source listing does not change.

```conduit compare
form same-morse-caller {
    message: text/literal("HELLO")
    morse: text/morse(40)
    light: presentation/indicator

    message > morse > light
}
```

A rich Host may use a direct Morse implementation. A smaller Host may open this Back until it reaches Gears it can provide. The caller still sees the same Face.
