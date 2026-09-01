# Meet the Host

A **Host** is a running environment that truthfully offers implementations and resources. The browser running this page is one Host.

Every example so far used its actual offers. Open the inventory only when you want to inspect what is available here.

<!-- conduit-host-inventory -->

The Form says what should happen. The Host says what machinery is available now.

# State over time

A Host can also offer bounded effects such as a monotonic timer. State and time remain visible Gears rather than hidden page behavior.

```conduit run
form count-over-time {
    count: state/count(start = 0)
    show: presentation/count(maximum-values = 5)
    clock: time/every(freq = 120ms)

    clock.tick > count.bump
    count.value > show.value
}
```

Run it and watch one finite Play count from 0 to 4.
