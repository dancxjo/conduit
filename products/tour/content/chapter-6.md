---
page: body-wide-realization
route: many-forms-one-body-wide-realization
companion: body-workload
---
# Many forms, one body-wide realization

In Conduit, **Program = form**.

A body begins with a bounded workset of zero, one, or many forms and may later add or remove forms without changing body identity. No initial form remains privileged.

```text
body Roseau
  forms:
    Patchbay
    music player
    background sync

host:
  8 CPU lanes
  memory
  display
  network
```

One body-wide admission model sees the whole workload. One wake is body-wide. One current immutable plan admits and places all currently carried forms together against all current resources. At most one body-wide play is active at once, while many gear instances from many forms may execute concurrently inside that play.

That is why two forms cannot independently reserve the same last CPU lane or device. Admission is shared because realization is shared.

Adding a Pico later changes topology, not ontology: a replacement body-wide plan may move compatible work there while the same body and form set continue.

For the one-machine case, ConduitOS is the freestanding host substrate. body scheduling plans ordinary forms against the exact processor, memory, and device resources that the current host can actually offer. Its cooperative kernel can make progress across two admitted execution regions; that is logical concurrency, with SMP and preemption still ahead. The eight-lane host above illustrates the model rather than claiming an available ConduitOS machine profile.
