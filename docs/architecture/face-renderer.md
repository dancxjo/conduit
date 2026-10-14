# Portable face renderer contract

Conduit face follows the same front/back law as every other semantic
kind:

```text
face -> face/renderer -> Manifestation
```

`face/renderer` is the portable front. Its checked front contains one
bounded `face` info input and one bounded `Manifestation` info
output. The kind name and front do not contain Wayland, DOM, SVG,
framebuffer, terminal, window, browser, or operating-system facts.

## face

A `face` is an immutable semantic content revision. It binds:

- the exact source and checked plot identities;
- optional expanded plot, plan, and active play identities as one coherent
  chain;
- a canonical finite sign identity set;
- finite semantic subjects, roles, relationships, labels, accessibility names,
  and subject-owned text.

The whole value has an aggregate byte ceiling in addition to count and
per-field bounds. Its content identity is derived from all semantic content.
Changing a label, relationship, exact identity, or revision changes the
content identity; reordering or duplicating canonical sign is rejected.

An optional finite `FaceInput` collection binds a semantic target,
typed value kind, byte ceiling, empty-value policy, accessible label, and
submit action. Human gestures cross the portable
`face/interaction` front only after an Available Manifestation binds
the exact face revision. Accepted interactions carry exact
face, Manifestation, input, action, target, type, sequence, and
payload identities; retained evidence records payload length but never its
plaintext. Stale, unavailable, malformed, duplicate, pressured, cancelled,
and adapter-failed cases remain distinct.

Coordinates, viewport, focus mechanics, toolkit objects, DOM identity, native
handles, pixel buffers, and base resources do not occur in this contract.
Those are renderer-local or planned realization state.

## Manifestation

A `Manifestation` is the portable result of realizing one exact face.
It binds the face identity/revision, plan, active play, renderer
placement, admitted output subject, lifecycle, and a finite typed Manifestation
sign chain.
Its identity is derived from the immutable correlation fields, including the
output subject.

The initial state is `Prepared`. Accepted transitions are:

```text
Prepared -> Available | Failed
Available -> Replaced | Closed | Failed
```

Every transition appends a sign built from the actual Manifestation,
face, plan, active play, placement, lifecycle, and failure values. Its
sign identity must not have appeared earlier in that lifecycle. Backward
transitions, duplicate signs, tampered correlation, stale
face/plan/placement correlation, invalid plans, and non-renderer
placements fail closed.

The value contains no raw surface, DOM, framebuffer, or pixel payload.

## Realizations and planning

hosts supply exact renderer offers beneath the shared checked front. An offer
names its implementation, artifact, execution profile, Host Call target,
resource class, and finite limits. For example, a Linux host may bind a
Wayland surface base while a browser host binds a DOM/SVG document
base. Those facts enter the resulting plan and produce different plan
identities; they do not rename the authored kind or gear.

A headless host is complete without advertising this optional capability. It
cannot invent a renderer merely because it can observe or transport a
face.

Renderer backs may themselves be plots when projection, layout, or rendering
steps add reusable semantic value. Decomposition ends at admitted face
Host Calls and resources. Wayland buffer commits, DOM mutation,
framebuffer writes, and terminal escapes remain base mechanisms rather
than a `machine/*` semantic catalog.

## Renderer self-inspection

Patchbay inspects the realization currently drawing Patchbay through one
bounded typed `RendererSelfInspection`. The value contains the actual validated
renderer plan and Manifestation; it does not reconstruct selected identities
from display strings. Both native and HTML surfaces derive the renderer front
and ports, placement, implementation and artifact, host and boot, resources,
finite limits, renderer plan and play, Manifestation lifecycle, and exact sign
chain from that same value.

The inspection is accepted only when the plan verifies, the Manifestation
validates against the exact face and plan, and exactly one
`face/renderer` placement matches the Manifestation placement. Missing,
ambiguous, stale, or tampered correlation fails closed. Renderer-local window,
DOM, geometry, focus, viewport, and theme state never enters this inspection.
