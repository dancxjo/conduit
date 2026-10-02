---
page: host-realization
journey: conduit-tour
route: hosts-make-plots-real
companion: host-inventory
stage: canonical-plot:count-over-time|run
---
# hosts make plots real

A **host** is a current running environment with finite truthful offers: implementations, resources, and effects. The browser running this Tour is itself a real host.

The plot says what should happen. The host says what machinery is available now.

<!-- conduit-host-inventory -->

hosts also provide effects over time. This example uses a timer and state gear so time remains explicit in the plot rather than hidden in page behavior.

```conduit run
plot count-over-time {
    count: state/count(start = 0)
    show: presentation/count
    clock: time/every(freq = 120ms)

    clock.tick >> count.bump
    count.value >> show.value
}
```

A **plan** is one exact admitted realization against current host offers and resources. A **play** is execution of that plan. Here the workload happens to contain one plot, but that is the smallest case of a later body-wide model, not a separate permanent plan/play universe per plot.
