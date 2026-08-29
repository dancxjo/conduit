# Step 0 — Hello, light

Suppose you want one small intention — turn this message into a visible signal — to survive the device that first demonstrates it. Today a browser indicator can show it. Tomorrow another suitable Host might use a physical light. If the program names the browser, GPIO pin, LED, operating system, or machine, changing the realization means rewriting the program.

## Conduit idea

A Form names the intended meaning and its composition, not the machinery that happens to realize it. Edit the message, press Run, and let this browser Host provide today's manifestation.

```conduit run
form hello-light {
    message: text/literal("SOS")
    morse: text/morse(120)
    light: presentation/indicator

    message > morse > light
}
```

## What the run proves

The Gears perform the semantic operations and each Cord carries one exact typed value. The checked Form asks for text, Morse timing, and an indication. It names neither a screen nor a physical light. This run proves the browser realization only; it does not pretend that another device ran.

## Payoff

Write the intention once. A different collection of capable machinery may realize that same meaning later without contaminating the Form with platform facts. That separation is the central promise the rest of the Tour makes precise.

# Step 1 — Change one Gear

Useful programs evolve. You should be able to replace one semantic piece without rewriting every neighbor or turning the whole program into platform-specific glue.

## Conduit idea

A Gear is one configured semantic operation. Its Face is the exact typed contract visible to the surrounding Form, so compatible pieces can be composed and changed locally.

```conduit run
form change-one-gear {
    message: text/literal("hello")
    change: text/upper
    result: presentation/text

    message > change > result
}
```

## What the run proves

Change the quoted text and Run again. The source, uppercase operation, and presentation remain ordinary typed pieces joined by Cords; the browser page does not perform a hidden uppercase shortcut.

## Payoff

Composition gives Conduit replaceable semantic pieces. The surrounding intention can remain stable while one part changes, which is the first ingredient needed for the same program to survive different realizations.

# Step 2 — Fan out explicitly

One value often needs to feed several branches. That becomes dangerous when those branches may eventually live on different finite machines: an implicit listener or hidden queue could accept one branch while silently losing or delaying another.

## Conduit idea

Fan-out is written as explicit Cords. A Plan must admit every branch and its finite capacity together before the emission can begin.

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

## What the run proves

Both visible branches receive the exact same scalar and reconverge at one comparison. Nothing is broadcast to an unnamed listener, and neither branch is an unbounded convenience queue.

## Payoff

The same graph can remain honest when its branches later cross Host boundaries. Conduit knows the whole finite obligation before work starts instead of discovering halfway through that only part of the intended computer can keep up.
