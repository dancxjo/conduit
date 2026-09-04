---
page: body-wide-realization
route: many-forms-one-body-wide-realization
companion: body-workload
---
# Many Forms, one Body-wide realization

In Conduit, **Program = Form**.

A Body begins with a bounded workset of zero, one, or many Forms and may later add or remove Forms without changing Body identity. No initial Form remains privileged.

```text
Body Roseau
  Forms:
    Patchbay
    music player
    background sync

Host:
  8 CPU lanes
  memory
  display
  network
```

One logical scheduler sees the whole Body workload. One Wake is Body-wide. One current immutable Plan admits and places all currently carried Forms together against all current resources. At most one Body-wide Play is active at once, while many Gear instances from many Forms may execute concurrently inside that Play.

That is why two Forms cannot independently reserve the same last CPU lane or device. Admission is shared because realization is shared.

Adding a Pico later changes topology, not ontology: a replacement Body-wide Plan may move compatible work there while the same Body and Form set continue.

For the one-machine case, ConduitOS is the OS-shaped freestanding Host substrate. It offers processor lanes, memory, storage, and devices as finite truthful resources; Body scheduling still plans ordinary Forms against those offers. Current proof remains explicit: ConduitOS currently proves one cooperative execution lane, not yet SMP or preemption.
