# Patchbay application

Patchbay is a Conduit application and projection over authoritative Form,
Plan, Play, Body, Host, Boot, Sign, and Observatory truth. It is not a Host,
planner, runtime, capability registry, or source of current realization truth.

- `model/` owns presentation-neutral application state and projections.
- `native/` owns the native renderer and hosted application composition edge.
- `html/` owns the bounded HTML delivery and browser renderer edge.

Concrete hosted bootstrap and platform effects belong at the renderer or
application-composition edge. The reusable model must consume exact
advertisements, plans, reports, and Observatory projections without depending
on `conduit-std-host` or reconstructing a second current-truth registry.
