# Use a generic verb

If every application and machine needs its own special operation — robot-scale-number, web-scale-number, sensor-scale-number — the vocabulary fragments and Forms stop being portable.

## Conduit idea

Prefer small generic semantic verbs whose typed meaning is useful across many domains. Exact scalar scaling is the same idea whether a later Form uses it for a robot, a visualization, or a sensor.

```conduit run
form generic-scale {
    source: scalar/literal(1.5)
    scale: math/scale(2.0)
    result: presentation/scalar

    source > scale > result
}
```

## What the run proves

The planned operation produces 3.000000 through the ordinary browser Host path. The exact six-place result comes from the selected implementation; there is no page-local arithmetic shortcut.

## Payoff

A compact reusable vocabulary can travel across unlike applications and Hosts. Conduit does not need a new platform-flavored Gear each time the same meaning appears somewhere else.

# A Gear can have a Back

Callers need useful high-level meaning, but they should not have to copy its internal machinery into every Form. Otherwise sophisticated operations make every caller larger and more dependent on how one Host implements them.

## Conduit idea

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

## What the run proves

Run the listing normally. The caller asks only for the text/morse Face, and this capable browser Host satisfies it with its available direct leaf. The reviewed Back remains implementation knowledge rather than source the caller must own.

## Payoff

Rich meaning can be assembled from simpler capabilities without exposing that assembly to every caller. The Face stays stable while Conduit gains another honest way to realize it.

# Morse opens up

A current Host may know a high-level operation directly, while a smaller Host knows only the pieces from which that operation can be built. Requiring every Host to implement the whole vocabulary natively would make constrained systems unnecessarily large.

## Conduit idea

The reviewed Back for Morse expresses text/morse through character decomposition, lookup, gaps, flattening, and timing. A planner may keep opening reviewed Backs until the remaining leaves are operations the available machinery actually offers.

```conduit run
form morse-opens-up {
    message: text/literal("SOS")
    encode: text/morse(unit-ms = 40)
    result: presentation/indicator

    message > encode > result
}
```

## What the run proves

This ordinary run still chooses the browser's direct text/morse leaf. The caller does not request a recursive mode, and the Tour does not silently force one. The important distinction is now visible: a Kind and its Face fix the meaning; a particular implementation does not.

## Payoff

The same high-level request can fit both rich and constrained Hosts. A later page places the direct leaf and recursive Back side by side so you can inspect why those two realization shapes exist.
