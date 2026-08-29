# Step 7 — State over time

A startup value becomes current state only as admitted values flow through the same finite Play.

```conduit run
form count-over-time {
    count: state/count(start = 0)
    show: presentation/count(maximum-values = 5)
    clock: time/every(freq = 120ms)

    clock.tick > count.bump
    count.value > show.value
}
```

The browser first shows the startup count, then four planned ticks advance the current count to 4. The Plan admits exactly one timer slot and a finite number of values before Play starts; the browser timer is an ordinary best-effort Host operation, not a hard real-time claim.

# Step 8 — Meet the Host

A Host is the running environment whose current truthful offers let the planner choose exact implementations.

<!-- conduit-host-inventory -->

Available entries come from the same browser Host advertisement used to plan every listing above. Catalog meaning can exist without an installed implementation; editing a Form to require one still refuses before Play.
