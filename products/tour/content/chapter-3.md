---
page: host-realization
route: hosts-make-forms-real
companion: host-inventory
stage: canonical-form:count-over-time|run
---
# Hosts make Forms real

A **Host** is a current running environment with finite truthful offers: implementations, resources, and effects. The browser running this Tour is itself a real Host.

The Form says what should happen. The Host says what machinery is available now.

<!-- conduit-host-inventory -->

Hosts also provide effects over time. This example uses a timer and state Gear so time remains explicit in the Form rather than hidden in page behavior.

```conduit run
form count-over-time {
    count: state/count(start = 0)
    show: presentation/count(maximum-values = 5)
    clock: time/every(freq = 120ms)

    clock.tick > count.bump
    count.value > show.value
}
```

A **Plan** is one exact admitted realization against current Host offers and resources. A **Play** is execution of that Plan. Here the workload happens to contain one Form, but that is the smallest case of a later Body-wide model, not a separate permanent Plan/Play universe per Form.
