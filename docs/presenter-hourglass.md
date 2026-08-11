# Presenter hourglass

Patchbay presentation has one canonical narrow waist:

```text
Presentation
  exact basis, subjects, relationships, labels, accessible names,
  semantic controls, lens-relevant facts, diagnostics, and provenance
                         |
                  presentation Face
                         |
        +----------------+----------------+
        |                |                |
     browser           native           linear
     DOM/SVG/CSS       raster/Wayland   structured text
                         |
                         +-- optional ordinary Form/Back expansion
                              layout and composition meanings
                              admitted graphics meanings
                              exact display implementation and resources
```

The waist is the bounded, renderer-neutral `conduit_presentation::Presentation`
value and its exact `PresentationBasis`. It is not a pixel surface, scene graph,
DOM tree, widget hierarchy, callback set, or renderer-local layout. A renderer
node that depicts a Gear does not become that Gear.

## Identities and realization

These identities remain distinct:

```text
Presentation meaning and content identity
!= presenter implementation and artifact identity
!= optional presenter Form/Back identity
!= expanded layout, composition, and graphics placements
!= display, browser-document, Wayland-surface, or framebuffer resource
!= renderer-local objects
```

The same Presentation may therefore reach several materially different
presenters. Each presenter joins at the highest seam it can truthfully satisfy:

- the browser implements the presentation Face directly with DOM/SVG/CSS and
  browser accessibility mechanisms;
- the native presenter implements the same Face directly with its own bounded
  layout and raster work;
- the deterministic linear presenter consumes the same value without claiming
  two-dimensional geometry;
- a constrained presenter may open an ordinary canonical Form/Back and realize
  admitted layout, composition, and graphics operations recursively.

The linear path is a complete nonvisual presentation, not a fake framebuffer.
Direct presenters do not advertise lower layers they do not implement.

## Ordinary recursive realization

A recursive presenter uses the ordinary Conduit path:

```text
canonical Form/Back expansion
-> checking
-> exact planning
-> lowering and preparation
-> production conduit-kernel execution
-> admitted Host operations and resources
-> bounded Signs
```

There is no renderer scheduler, private recursive executor, or second semantic
graph. The exact expanded Form and Plan record every selected Back, leaf
implementation, Host, Boot, operation, and resource. Direct and recursive Plans
must differ because their realization differs; the presented user Form and its
presentation meaning do not change.

The mechanically generated `cargo xtask catalog matrix` report is the static
coverage view. It distinguishes the direct browser implementation from the
installed constrained recursive realization and does not promote installed
coverage into a claim about a current Boot.

## Admission below the waist

A drawing or layout operation becomes portable meaning only when two materially
different presenters can implement its exact bounded contract without sharing
toolkit internals. This admits reusable geometry, clipping, composition, text,
and icon intent when they have independent semantic value. Raster helpers,
glyph caches, callbacks, DOM identifiers, widget objects, framebuffer addresses,
and private line/path helpers stay inside their presenter.

The default Patchbay canvas presents the user's program. Recursive realization
is available for explicit inspection; it is not injected into the maker-facing
graph merely because Patchbay can inspect its own Plan.

## Interaction and failure

Meaningful input returns through the same semantic interaction boundary as
presentation output. Pointer, keyboard, touch, hit testing, and focus are local
mechanisms; they produce the bounded interaction requests owned by #694. A
presenter Back does not gain edit authority from owning geometry.

Renderer failures remain realization facts. A lost browser document, native
surface, display resource, font/icon implementation, or recursive leaf cannot
rewrite the Presentation or the user's Form. Replanning or fallback is permitted
only through ordinary Plan rules and exact current offers. A still-available
linear presenter remains a separate truthful realization, not evidence that a
failed graphical realization succeeded.

## Conformance proof

Cross-presenter conformance compares the exact Presentation basis and normalized
subjects, relationships, labels, accessible names, properties, text, controls,
selection/lens-relevant facts, and diagnostics. It deliberately does not compare
pixels, coordinates, typography, wrapping, toolkit nodes, or screenshots.

`renderer_execution_tests::native_browser_and_linear_presenters_preserve_one_exact_semantic_specimen`
feeds one bounded Patchbay specimen to the native, browser, and deterministic
linear paths. `presenter_capstone_tests` separately proves that one unchanged
high-level Patchbay meaning selects distinct direct and recursive Plans, lowers
both through production machinery, executes both with finite Signs, preserves
the portable interaction seam, and reports realization-specific absence without
mutating source meaning.
