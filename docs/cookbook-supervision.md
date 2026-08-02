# Typed supervision cookbook

Use supervision when an already-admitted node or composite cannot continue.
Keep expected negative domain results on the domain contract's ordinary
ports, and handle compile/admission diagnostics outside the run.

The source relationship is intentionally small:

State: **contract-only**. Exact policy, implementation, host, authority,
timer, and allocation bindings are deliberately absent from this snippet.

```panel
panel 0

request: std/literal {
    value = "work"
}
output: display/text
request_policy: supervision/supervisor

request.value > output.text
supervise request with request_policy
```

`supervise` does not choose retry counts or create a fallback. Exact compile
input must bind this source relationship to a finite policy, implementation,
host, authority set, actions, timers, and allocation. For example, a planner
may admit `propagate` and two uses of `restart-same`; it may not let the
handler discover a replacement or grant itself authority.

A fallback is a separate already-resolved compatible node in the same exact
plan. If selecting it changes implementation, topology, artifact, authority,
or epoch beyond that admitted choice, prepare an issue-57 candidate transition
instead.

The complete contract, stable reasons, resource equation, and evidence rules
are in [typed supervision version 1](../spec/049-typed-supervision.md).
`conformance/c4/supervision.json` contains positive, negative, race,
capacity, source, browser, and constrained-profile examples.
