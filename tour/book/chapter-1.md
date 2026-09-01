# Meet one Gear

A **Gear** does one piece of work. Here, `change` turns text into uppercase.

Change the quoted words, then Run. Watch the value move from the first Gear, through `change`, to the result.

```conduit run
form meet-one-gear {
    words: text/literal("hello")
    change: text/upper
    result: presentation/text

    words > change > result
}
```

That is enough for a useful start: a small operation you can see, edit, and run.

# Connect Gears

Gears become a Form when **Cords** connect their typed Ports. This one carries text into a Morse encoder, then carries the pattern to an indicator.

```conduit run
form connect-gears {
    message: text/literal("E")
    morse: text/morse(40)
    light: presentation/indicator

    message > morse > light
}
```

Run it once. The Patchbay is the Form: Gears are the plates and Cords are the lines between them.

# Edit one Gear

The listing and Patchbay describe the same Form. Edit the literal without rebuilding the rest of the graph.

```conduit run
form edit-one-gear {
    words: text/literal("make this loud")
    change: text/upper
    result: presentation/text

    words > change > result
}
```

The neighboring Gears do not need to know how the words changed. They only need compatible Ports.
