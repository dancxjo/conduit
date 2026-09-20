---
page: multi-host-form
route: one-form-across-several-hosts
companion: multi-host-plan
stage: canonical-form:hello-across|two-host
stage: canonical-form:hello-across|two-host-plan
---
# One form across several hosts

Now keep the same form semantics but realize it across two hosts.

```conduit run two-host
form hello-across {
    message: text/literal("hello across one cord")
    show: presentation/text

    message > show
}
```

A cross-host connection keeps its semantic cord identity. The current connectivity chosen to realize that cross-host cord is a **line**.

This exercise is **browser-runtime proof**: two independent, ephemeral browser
hosts exchange bounded values over one in-memory line selected by the plan. It
does not admit either host as a durable part, persist a body, grant trust or
effect authority, prove rejoin after restart, or establish a household. The
[House roadmap](../../../docs/roadmap.md#the-house-and-a-physical-laptop)
requires those separate continuity and admission results.

This is not "client code plus server code". It is one unchanged form with gear placement decisions in an exact realization.

```conduit run two-host plan
form hello-across {
    message: text/literal("hello across one cord")
    show: presentation/text

    message > show
}
```

Inspect exact evidence to see placement, line facts, and resource admission identities. Those exact IDs are intentionally secondary until evidence itself is the topic.
