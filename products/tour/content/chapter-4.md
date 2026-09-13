---
page: multi-host-form
route: one-form-across-several-hosts
companion: multi-host-plan
stage: canonical-form:hello-across|two-host
stage: canonical-form:hello-across|two-host-plan
---
# One Form across several Hosts

Now keep the same Form semantics but realize it across two Hosts.

```conduit run two-host
form hello-across {
    message: text/literal("hello across one Cord")
    show: presentation/text

    message > show
}
```

A cross-Host connection keeps its semantic Cord identity. The current connectivity chosen to realize that cross-Host Cord is a **Line**.

This exercise is **browser-runtime proof**: two independent, ephemeral browser
Hosts exchange bounded values over one in-memory Line selected by the Plan. It
does not admit either Host as a durable Part, persist a Body, grant trust or
effect authority, prove rejoin after restart, or establish a household. The
[House roadmap](../../../docs/roadmap.md#the-house-and-a-physical-laptop)
requires those separate continuity and admission results.

This is not "client code plus server code". It is one unchanged Form with Gear placement decisions in an exact realization.

```conduit run two-host plan
form hello-across {
    message: text/literal("hello across one Cord")
    show: presentation/text

    message > show
}
```

Inspect exact evidence to see placement, Line facts, and resource admission identities. Those exact IDs are intentionally secondary until evidence itself is the topic.
