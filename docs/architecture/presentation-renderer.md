# Portable presentation renderer contract

Conduit presentation follows the same Face/Back law as every other semantic
Kind:

```text
Presentation -> presentation/renderer -> Manifestation
```

`presentation/renderer` is the portable Face. Its checked Face contains one
bounded `Presentation` Info input and one bounded `Manifestation` Info
output. The Kind name and Face do not contain Wayland, DOM, SVG,
framebuffer, terminal, window, browser, or operating-system facts.

## Presentation

A `Presentation` is an immutable semantic content revision. It binds:

- the exact source and checked Form identities;
- optional expanded Form, Plan, and active Play identities as one coherent
  chain;
- a canonical finite Sign identity set;
- finite semantic subjects, roles, relationships, labels, accessibility names,
  and subject-owned text.

The whole value has an aggregate byte ceiling in addition to count and
per-field bounds. Its content identity is derived from all semantic content.
Changing a label, relationship, exact identity, or revision changes the
content identity; reordering or duplicating canonical Sign is rejected.

An optional finite `PresentationInput` collection binds a semantic target,
typed value Kind, byte ceiling, empty-value policy, accessible label, and
submit action. Human gestures cross the portable
`presentation/interaction` Face only after an Available Manifestation binds
the exact Presentation revision. Accepted interactions carry exact
Presentation, Manifestation, input, action, target, type, sequence, and
payload identities; retained evidence records payload length but never its
plaintext. Stale, unavailable, malformed, duplicate, pressured, cancelled,
and adapter-failed cases remain distinct.

Coordinates, viewport, focus mechanics, toolkit objects, DOM identity, native
handles, pixel buffers, and base resources do not occur in this contract.
Those are renderer-local or planned realization state.

## Manifestation

A `Manifestation` is the portable result of realizing one exact Presentation.
It binds the Presentation identity/revision, Plan, active Play, renderer
placement, admitted output subject, lifecycle, and a finite typed Manifestation
Sign chain.
Its identity is derived from the immutable correlation fields, including the
output subject.

The initial state is `Prepared`. Accepted transitions are:

```text
Prepared -> Available | Failed
Available -> Replaced | Closed | Failed
```

Every transition appends a Sign built from the actual Manifestation,
Presentation, Plan, active Play, placement, lifecycle, and failure values. Its
Sign identity must not have appeared earlier in that lifecycle. Backward
transitions, duplicate Signs, tampered correlation, stale
Presentation/Plan/placement correlation, invalid Plans, and non-renderer
placements fail closed.

The value contains no raw surface, DOM, framebuffer, or pixel payload.

## Realizations and planning

Hosts supply exact renderer offers beneath the shared checked face. An offer
names its implementation, artifact, execution profile, host-operation target,
resource class, and finite limits. For example, a Linux host may bind a
Wayland surface base while a browser host binds a DOM/SVG document
base. Those facts enter the resulting Plan and produce different Plan
identities; they do not rename the authored Kind or Gear.

A headless host is complete without advertising this optional capability. It
cannot invent a renderer merely because it can observe or transport a
Presentation.

Renderer Backs may themselves be Forms when projection, layout, or rendering
steps add reusable semantic value. Decomposition ends at admitted presentation
host operations and resources. Wayland buffer commits, DOM mutation,
framebuffer writes, and terminal escapes remain base mechanisms rather
than a `machine/*` semantic catalog.

## Renderer self-inspection

Patchbay inspects the realization currently drawing Patchbay through one
bounded typed `RendererSelfInspection`. The value contains the actual validated
renderer Plan and Manifestation; it does not reconstruct selected identities
from display strings. Both native and HTML surfaces derive the renderer Face
and Ports, placement, implementation and artifact, Host and Boot, resources,
finite limits, renderer Plan and Play, Manifestation lifecycle, and exact Sign
chain from that same value.

The inspection is accepted only when the Plan verifies, the Manifestation
validates against the exact Presentation and Plan, and exactly one
`presentation/renderer` placement matches the Manifestation placement. Missing,
ambiguous, stale, or tampered correlation fails closed. Renderer-local window,
DOM, geometry, focus, viewport, and theme state never enters this inspection.
