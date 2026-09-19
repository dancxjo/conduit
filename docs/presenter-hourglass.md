# Presenter hourglass

Patchbay presentation has one canonical narrow waist:

```text
presentation
  exact basis, subjects, relationships, labels, accessible names,
  semantic controls, lens-relevant facts, diagnostics, and provenance
                         |
                  presentation face
                         |
        +----------------+----------------+
        |                |                |
     browser           native           linear
     DOM/SVG/CSS       raster/Wayland   structured text
                         |
                         +-- optional ordinary form/back expansion
                              layout and composition meanings
                              admitted graphics meanings
                              exact display implementation and resources
```

The waist is the bounded, renderer-neutral `conduit_presentation::presentation`
value and its exact `PresentationBasis`. It is not a pixel surface, scene graph,
DOM tree, widget hierarchy, callback set, or renderer-local layout. A renderer
node that depicts a gear does not become that gear.

## Identities and realization

These identities remain distinct:

```text
presentation meaning and content identity
!= presenter implementation and artifact identity
!= optional presenter form/back identity
!= expanded layout, composition, and graphics placements
!= display, browser-document, Wayland-surface, or framebuffer resource
!= renderer-local objects
```

The same presentation may therefore reach several materially different
presenters. Each presenter joins at the highest seam it can truthfully satisfy:

- the browser implements the presentation face directly with DOM/SVG/CSS and
  browser accessibility mechanisms;
- the native presenter implements the same face directly with its own bounded
  layout and raster work;
- the deterministic linear presenter consumes the same value without claiming
  two-dimensional geometry;
- a constrained presenter may open an ordinary canonical form/back and realize
  admitted layout, composition, and graphics operations recursively.

The linear path is a complete nonvisual presentation, not a fake framebuffer.
Direct presenters do not advertise lower layers they do not implement.

## Ordinary recursive realization

A recursive presenter uses the ordinary Conduit path:

```text
canonical form/back expansion
-> checking
-> exact planning
-> lowering and preparation
-> production conduit-kernel execution
-> admitted host operations and resources
-> bounded signs
```

There is no renderer scheduler, private recursive executor, or second semantic
graph. The exact expanded form and plan record every selected back, leaf
implementation, host, boot, operation, and resource. Direct and recursive plans
must differ because their realization differs; the presented user form and its
presentation meaning do not change.

The mechanically generated `cargo xtask catalog matrix` report is the static
coverage view. It distinguishes the direct browser implementation from the
installed constrained recursive realization and does not promote installed
coverage into a claim about a current boot.

## Admission below the waist

A drawing or layout operation becomes portable meaning only when two materially
different presenters can implement its exact bounded contract without sharing
toolkit internals. This admits reusable geometry, clipping, composition, text,
and icon intent when they have independent semantic value. Raster helpers,
glyph caches, callbacks, DOM identifiers, widget objects, framebuffer addresses,
and private line/path helpers stay inside their presenter.

The default Patchbay canvas presents the user's program. Recursive realization
is available for explicit inspection; it is not injected into the maker-facing
graph merely because Patchbay can inspect its own plan.

## Interaction and failure

Meaningful input returns through the same semantic interaction boundary as
presentation output. Pointer, keyboard, touch, hit testing, and focus are local
mechanisms; they produce the bounded interaction requests owned by #694. A
presenter back does not gain edit authority from owning geometry.

Renderer failures remain realization facts. A lost browser document, native
surface, display resource, font/icon implementation, or recursive leaf cannot
rewrite the presentation or the user's form. Replanning or fallback is permitted
only through ordinary plan rules and exact current offers. A still-available
linear presenter remains a separate truthful realization, not evidence that a
failed graphical realization succeeded.

## Conformance proof

Cross-presenter conformance compares the exact presentation basis and normalized
subjects, relationships, labels, accessible names, properties, text, controls,
selection/lens-relevant facts, and diagnostics. It deliberately does not compare
pixels, coordinates, typography, wrapping, toolkit nodes, or screenshots.

`renderer_execution_tests::native_browser_and_linear_presenters_preserve_one_exact_semantic_specimen`
feeds one bounded Patchbay specimen to the native, browser, and deterministic
linear paths. `presenter_plans_tests` separately proves that one unchanged
high-level Patchbay meaning selects distinct direct and recursive plans, lowers
both through production machinery, executes both with finite signs, preserves
the portable interaction seam, and reports realization-specific absence without
mutating source meaning.
