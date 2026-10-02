---
page: body-wide-realization
journey: conduit-tour
route: many-plots-one-body-wide-realization
companion: body-workload
---
# Many plots, one body-wide realization

In Conduit, **Program = plot**.

A body begins with a bounded workset of zero, one, or many plots and may later add or remove plots without changing body identity. No initial plot remains privileged.

```text
body Roseau
  plots:
    Patchbay
    music player
    background sync

host:
  8 CPU lanes
  memory
  display
  network
```

One body-wide admission model sees the whole workload. One wake is body-wide. One current immutable plan admits and places all currently carried plots together against all current resources. At most one body-wide play is active at once, while many gear instances from many plots may execute concurrently inside that play.

That is why two plots cannot independently reserve the same last CPU lane or device. Admission is shared because realization is shared.

Adding a Pico later changes topology, not ontology: a replacement body-wide plan may move compatible work there while the same body and plot set continue.

For the one-machine case, ConduitOS is the freestanding host substrate. body scheduling plans ordinary plots against the exact processor, memory, and device resources that the current host can actually offer. Its cooperative kernel can make progress across two admitted execution regions; that is logical concurrency, with SMP and preemption still ahead. The eight-lane host above illustrates the model rather than claiming an available ConduitOS machine profile.
