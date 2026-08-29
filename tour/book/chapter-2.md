# Step 3 — Use a generic verb

A small generic verb such as exact scalar scaling composes without inventing a new domain-specific Gear.

```conduit run
form generic-scale {
    source: scalar/literal(1.5)
    scale: math/scale(2.0)
    result: presentation/scalar

    source > scale > result
}
```

The planned result retains six exact decimal places; there is no page-local arithmetic shortcut.

# Step 4 — A Gear can have a Back

A high-level Gear keeps one public Face while a reviewed Form Back can describe the same meaning with smaller Gears.

```text
             text/morse
                 |
          one checked Face
                 |
       reviewed Form Back
                 |
 characters -> lookup -> gaps -> flatten -> timing
```

```conduit run
form gear-with-a-back {
    message: text/literal("E")
    morse: text/morse(40)
    light: presentation/indicator

    message > morse > light
}
```

Run the listing normally. The caller asks for the Face while implementation knowledge remains hidden behind it.

# Step 5 — Morse opens up

The reviewed Back for Morse expresses `text/morse` through reusable character, lookup, gap, flatten, and timing Gears while its meaning stays unchanged.

```conduit run
form morse-opens-up {
    message: text/literal("SOS")
    encode: text/morse(unit-ms = 40)
    result: presentation/indicator

    message > encode > result
}
```

This still runs through ordinary planning. The Back is implementation knowledge, not a different operation for the caller to request. The decisive distinction is simple: a Kind is not its implementation.
