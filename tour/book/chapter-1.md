# Bodies begin somewhere

Conduit lets you build one computer out of the computers you actually have. The devices can be unlike, constrained, replaced, or temporarily unavailable while the intended computer continues.

We call that durable computer a **Body**. A Body has durable identity distinct from a friendly name, its current program, and the machines currently helping realize it.

## Conduit idea

Birth is an explicit beginning, not a documentation-navigation trick. A newborn Body begins **LULLED**: it exists and has durable identity, but it has not yet been given machinery or started work. The independent Crèche provides temporary bootstrap help; the Book only explains the idea.

```conduit birth
form morse_network {
    message: text/literal("SOS")
    morse: text/morse(120)
    light: presentation/indicator
    message > morse > light
}
```

## What the Crèche proves

Choose **Birth a Body** above when you want to perform the stateful workflow. The Crèche receipt separates the mutable friendly name, selected program, durable Body identity, and BIRTH evidence. It also shows that no Host, Wake, Plan, or Play was silently created.

## Payoff

The Book remains readable whether or not you create a Body. If you do, leaving the Crèche or returning here must not delete that Body or rewrite its beginning.

# Add a physical Host

## Conduit idea

A successful deployment is only deployment: a physical Host joins this Body only after its fresh Boot and invitation-bound request are separately observed and explicitly admitted.

The Crèche owns this stateful workflow. Each action there advances exactly one owner-controlled state. The unchanged Form remains free of board, USB, serial, address, implementation, spore, and deployment facts.

On Linux, install Conduit's narrow Pico device rules once with `sudo scripts/install-pico-headless-flash.sh`, then reconnect the Pico. The rule grants `plugdev` access only to the RP2040 BOOTSEL/Picoboot identity and Conduit's exact running-firmware identity.

<!-- conduit-physical-host -->

The final state admits current membership, offers, and readiness. It deliberately creates no Plan or Play; running the same semantic Form on the physical indicator belongs to a later proof.

## Payoff

The machine becomes part of the Body through attributable evidence instead of being trusted merely because a cable appeared or a flash command returned success.

# Change one Gear

A Body needs machinery before it can run. The first practical problem is to admit a machine that can help realize its program. The Crèche can offer its browser as the first Host without making the browser part of Body identity; the Book does not perform that admission.

<!-- conduit-first-host -->

Once the Body has a Host, useful programs will still evolve. You should be able to replace one semantic piece without rewriting every neighbor or turning the whole program into platform-specific glue.

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

# Fan out explicitly

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
