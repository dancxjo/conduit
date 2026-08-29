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

A high-level Gear keeps one public Face while a reviewed Form can provide its hidden realization.

```text
             text/morse
                 |
          one checked Face
                 |
       reviewed Form Back
                 |
 characters -> lookup -> gaps -> flatten -> timing
```

```conduit run recursive
form gear-with-a-back {
    message: text/literal("E")
    morse: text/morse(40)
    light: presentation/indicator

    message > morse > light
}
```

Run the listing, then open What happened? to see the exact Back and planned leaves without putting them in the caller.

# Step 5 — Morse opens up

The authored Morse Gear can expand through reusable character, lookup, gap, flatten, and timing Gears while its meaning stays unchanged.

```conduit run recursive
form morse-opens-up {
    message: text/literal("SOS 2")
    encode: text/morse(unit-ms = 40)
    decode: morse/text
    result: presentation/text

    message > encode > decode > result
}
```

The decisive distinction is simple: a Kind is not its implementation.
