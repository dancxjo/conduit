# State over time

Real programs remember things and react over time. If state and timers become special escape hatches, the program stops being bounded and portable precisely when it becomes useful.

## Conduit idea

State changes and time-driven values remain ordinary typed flow inside one finite Play. The Plan must admit their value capacity and Host-operation slots before execution starts.

```conduit run
form count-over-time {
    count: state/count(start = 0)
    show: presentation/count(maximum-values = 5)
    clock: time/every(freq = 120ms)

    clock.tick > count.bump
    count.value > show.value
}
```

## What the run proves

The browser first presents the startup count, then four planned ticks advance the current count to 4. The Plan admits exactly one timer slot and five presentations. The browser timer is an ordinary best-effort Host operation, not a hard real-time claim.

## Payoff

Stateful, time-aware behavior can participate in the same finite portable model as the earlier stateless examples. Moving that behavior to different suitable machinery does not require inventing a second runtime law.

# Meet the Host

The Form cannot survive different machines if it has to guess what each machine can do. A browser, laptop, ConduitOS system, and microcontroller may offer very different capabilities and have different resources available now.

## Conduit idea

A Host is one exact running environment. It advertises truthful finite offers; the planner selects only implementations and resources that the current Host/Boot can actually provide.

<!-- conduit-host-inventory -->

## What the page proves

These available entries come from the same browser Host advertisement used to plan every listing above. Catalog meaning can exist without an installed implementation, and editing a Form to require an unoffered Kind still refuses before Play.

## Payoff

Machine-specific truth stays with the machine instead of leaking into authored meaning. Different Hosts can contribute different capabilities, which is how one Form can remain unchanged while the collection of machinery beneath it changes.
