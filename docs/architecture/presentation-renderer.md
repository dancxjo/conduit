# Portable presentation renderer contract

Conduit presentation follows the same Front/Back law as every other semantic
operation:

```text
Presentation -> presentation/renderer -> Manifestation
```

`presentation/renderer` is the portable Front. Its checked face contains one
bounded `Presentation` value input and one bounded `Manifestation` value
output. The operation name and face do not contain Wayland, DOM, SVG,
framebuffer, terminal, window, browser, or operating-system facts.

## Presentation

A `Presentation` is an immutable semantic content revision. It binds:

- the exact source and checked Form identities;
- optional expanded Form, Plan, and active Play identities as one coherent
  chain;
- a canonical finite evidence identity set;
- finite semantic subjects, roles, relationships, labels, accessibility names,
  and subject-owned text.

The whole value has an aggregate byte ceiling in addition to count and
per-field bounds. Its content identity is derived from all semantic content.
Changing a label, relationship, exact identity, or revision changes the
content identity; reordering or duplicating canonical evidence is rejected.

Coordinates, viewport, focus mechanics, toolkit objects, DOM identity, native
handles, pixel buffers, and provider resources do not occur in this contract.
Those are renderer-local or planned realization state.

## Manifestation

A `Manifestation` is the portable result of realizing one exact Presentation.
It binds the Presentation identity/revision, Plan, active Play, renderer
placement, admitted output subject, lifecycle, and current evidence identity.
Its identity is derived from the immutable correlation fields, including the
output subject.

The initial state is `Prepared`. Accepted transitions are:

```text
Prepared -> Available | Failed
Available -> Replaced | Closed | Failed
```

Every transition requires a new evidence identity. Backward transitions,
duplicate evidence, stale Presentation/Plan/placement correlation, invalid
Plans, and non-renderer placements fail closed.

The value contains no raw surface, DOM, framebuffer, or pixel payload.

## Realizations and planning

Hosts supply exact renderer offers beneath the shared checked face. An offer
names its implementation, artifact, execution profile, host-operation target,
resource class, and finite limits. For example, a Linux host may bind a
Wayland surface provider while a browser host binds a DOM/SVG document
provider. Those facts enter the resulting Plan and produce different Plan
identities; they do not rename the authored operation.

A headless host is complete without advertising this optional capability. It
cannot invent a renderer merely because it can observe or transport a
Presentation.

Renderer Backs may themselves be Forms when projection, layout, or rendering
steps add reusable semantic value. Decomposition ends at admitted presentation
host operations and resources. Wayland buffer commits, DOM mutation,
framebuffer writes, and terminal escapes remain provider mechanisms rather
than a `machine/*` semantic catalog.
