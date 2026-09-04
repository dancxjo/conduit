# Native Patchbay primitive GUI

Issue #685 replaces the native Form document as the only manifestation with a small graphical
Patchbay composition. The deterministic document remains available with `F2`; both views consume
the same checked/expanded Form facts.

## Primitive-layer decision

The bounded spike was performed on 2026-08-09 against the published crates and upstream source.

| Candidate | std / no_std / alloc | Rendering and input | Icons, layout, clipping | Bounds, unsafe, maturity | Custom #685 work left | Decision |
|---|---|---|---|---|---|---|
| `embedded-gui` 0.1.6 | `no_std` core, but defaults enable `std` and `rich-widgets`; Rust 1.87 | optional `embedded-graphics`; pointer, encoder, and keyboard event routing with focus/capture/bubble | broad buttons, icon buttons, panels, lists, scroll views, layout, and clipping | explicitly fixed-capacity; six reviewed `unsafe` pin/buffer operations; release published 2026-08-09 and therefore too new to supply long-lived project evidence | all Gear/Port/Cord geometry, exact selection, inspector, and Patchbay composition | rejected for this slice because adopting the broad application/widget surface would remove little Patchbay-specific code |
| `kolibri-embedded-gui` 0.1.0 | `no_std`; heapless state and optional caller-provided frame buffer; no published MSRV | immediate mode over `embedded-graphics::DrawTarget`; pointer interaction | buttons, labels, icon buttons, linear layout, and incremental redraw; icon dependency enables its full resolution set | two reviewed `unsafe` frame-buffer accesses; first and only published release dates to 2025-02-25; small and promising, but not yet a mature compatibility surface | nearly all exact graph interaction plus keyboard focus and inspector composition | rejected for this slice because the existing pinned drawing seam already covered the useful substrate without adding another icon/theme model |
| Slint 1.17.1 | software renderer can target embedded systems, but the current crate defaults to `std`, backend selection, accessibility, system tray, and multiple renderers; Rust 1.92 | declarative generated UI plus a substantial runtime/backend model | mature widgets, focus, layout, clipping, scrolling, accessibility | public facade scan found no production `unsafe` block, though its internal runtime/backends are a much larger dependency surface; mature 1.x project, inspected release published 2026-07-07 | graph canvas remains custom and a second generated UI architecture would need fencing | rejected as too much architecture for the stop line |
| GPUI (Zed) | desktop `std` application framework, not a shared `no_std` substrate | retained elements, actions, focus handles, and platform rendering | strong desktop layout/focus/action ideas | production desktop framework with platform/runtime unsafe machinery and no fixed-capacity contract; mature in Zed, but not released as a small stable embedded substrate | a separate framebuffer renderer and all shared Patchbay semantics remain necessary | studied for focus/action vocabulary only; rejected as the implementation substrate |
| existing custom seam | native `std` renderer over the already pinned `embedded-graphics` 0.8.1; drawing helpers remain compatible with a finite `DrawTarget` | direct `winit` pointer/keyboard events; finite renderer-local hit targets | 25 custom geometry icons, frames, labels, buttons, fixed regions, clipped text, nodes, jacks, and elbow Cords | every collection is bounded by the canonical graph limits; no retained widget tree, cache, or new unsafe code | only the Patchbay-specific code required by #685 | chosen |

The decision does not forbid reevaluating a crate when scrolling, editing, or framebuffer work
requires enough generic machinery to change this balance.

## Typed boundary

`patchbay-model::PatchbayGraph` is built from one validated `ExpandedCanonicalForm`. It carries the
exact source-document, checked-Form, and expanded-Form identities; primitive Gear and Kind
identities; exact directional typed Port descriptors; and Cords derived only from admitted exact
connection endpoints. The model rejects missing endpoints and collections beyond its fixed Gear,
Port, and Cord limits. Its inspector accepts only identities already present in that projection.

Native-only state consists of pixel layout, the cursor, selected subject index, linear/graphical
disclosure, and a finite hit-target table. Pointer hits choose an existing typed subject or one of
three explicit UI actions. They cannot create graph identities. Resize, repaint, selection, theme,
and panel layout do not enter source, checked, expanded, Plan, or Play identity.

Every graph hit stores a `PatchbaySubjectRef` containing both the admitted subject identity and the
exact expanded-Form identity from which its geometry was built. Applying a target to a replacement
projection fails as `StaleGraphBasis`; a fabricated subject fails separately as `UnknownSubject`.
These are pre-admission candidates for #694, not operation-success records.

The selected-subject index and current action dispatch are provisional integration state, not an
accepted semantic boundary. Issue #694 still owns the typed platform-neutral interaction path. This
GUI branch must remain draft and must not merge while pointer/keyboard selection can change the
inspector directly; completion requires rebasing onto #694 and proving selection plus at least one
meaningful control through that ordinary admitted path. Hit success alone is not operation success.

## Native composition

The first composition contains:

- a labeled and icon-bearing `FORM -> BODY -> WAKE -> PLAN -> PLAY` strip whose status comes from
  the existing Body/Wake and Plan/Play controllers;
- framed Open Back, Save, and linear-view action targets;
- a navigator, central canvas, inspector, and status region;
- at least three primitive Gears for `forms/hello/main.conduit`, including the checked literal Gear;
- explicit typed input/output jacks and elbow Cords between their exact endpoints;
- pointer selection and wraparound arrow-key traversal across Gears, Ports, and Cords;
- a selected-subject inspector with exact identity, Kind/Port/Info facts, and wrapped exact
  source/checked/expanded identity basis; and
- the prior bounded Unicode bitmap text path as one drawing primitive and as the `F2` linear view.

The software `Context` and `Surface` live for the window lifetime. Dropping them after each paint
can log a successful buffer commit while leaving a real Wayland client blank, so actual compositor
smoke is part of this contract rather than an in-memory pixel substitute.

This layer is not a widget toolkit, Form editor redesign, planner, lifecycle controller, runtime,
Host registry, or semantic layout model. It adds no drag-to-rewire, docking, animation, GPU
renderer, proportional typography, or renderer-owned execution truth.
