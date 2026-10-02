# Face, Mask, and Show

Conduit has one canonical human-semantic waist:

```text
Body/application truth --------+
resident Plot contributions ---+
Space/relationship truth ------+
tutorial and inspection -------+
                                v
                              Face
                                |
                  +-------------+-------------+
                  |             |             |
             graphical       spoken       generative/...
               Mask           Mask           Mask
                  |             |             |
                  +------------ Show ----------+
```

The waist is **Face**: bounded, renderer-neutral humanly relevant semantic
truth and control. The current Rust implementation is the migration-era
`conduit_presentation::Presentation` value; that type name is not a second
architectural altitude.

```text
basis and provenance
interaction context
subjects and semantic roles
relationships
properties
typed content
human wording
actions and bounded inputs
disclosure
projection and navigation
semantic order
semantic composition
time
```

It is not a pixel surface, scene graph, DOM tree, widget hierarchy, callback
set, spoken script, LLM prompt, or renderer-local layout. A local object that
depicts a gear does not become that gear.

`PresentationMechanism`, `SemanticApplicationView`, and `ApplicationView` are
useful finite UI-oriented vocabularies and compatibility adapters. They are
downstream from the waist. `Panel`, `Grid`, `Button`, `PatchbayCanvas`, and
similar compositions do not define what Face can mean.

## Universal semantics, open domains

Subject roles and relationship meanings have exact validated semantic
identities. Generic masks must preserve an unfamiliar valid identity and can
fall back to its human name, wording, and disclosure. Conduit does not grow a
central enum every time a plot presents a new scientific, educational, social,
musical, spatial, or robotic concept.

Typed content associates a subject with exact finite semantic data. The data
may be a diagram, image, audio excerpt, structured model, or document, but the
association names no file path, URL, image element, provider resource, or
framebuffer. Planning and the selected mask own how—or whether—it is realized.

When order is meaning, Face states it explicitly. Vector order,
serialization order, x/y position, DOM position, and spoken delivery order are
not semantic order by accident.

Projection identifies the truth relevant to the current encounter. Semantic
composition states the renderer-neutral rhetoric among that truth—grouping,
contrast, juxtaposition, emphasis, subordination, association, and reveal
relation. Mere co-presence is already expressed by projection membership;
composition states why the meanings belong together. Masks may realize that
rhetoric as slide regions, panes,
typography, speech structure, braille grouping, or another medium, but those
mechanisms never enter the universal grammar.

Accessibility is not another presentation mode. Face supplies ordinary
human names and descriptions. Graphical, spoken, braille, browser, and future
masks realize that same semantic truth through their respective media.

## Context and portable navigation

One Body may compose several distinct Face projections for several exact
interaction contexts. The context identity participates in Face
identity and stale-safe interaction correlation; it is not invisible audience
state owned by a Mask or Host. Domain meanings such as teacher, participant, or
room remain domain truth rather than universal presentation roles.

Portable navigation retains four independent axes:

```text
Place x Aspect x Focus x Depth
```

This is semantic projection, not layout. `ENTER`, `SHOW`, `FOCUS`, `FOLLOW`,
`DISCLOSE`, and `BACK` alter navigation. `INVOKE` requests an action. A spoken
Mask and a graphical workbench consume the same navigation truth even though
their local mechanics are materially different.

## Identities and realization

These identities remain distinct:

```text
Face meaning and content identity
!= mask plot source/checked/expanded identity
!= mask plan, implementation, and artifact identity
!= Show identity
!= expanded layout, composition, and graphics placements
!= display, browser-document, Wayland-surface, or framebuffer resource
!= mask-local objects
```

The same Face may therefore reach several materially different
masks. Each mask joins at the highest seam it can truthfully satisfy:

- a browser mask implements the Face Fore directly with DOM/SVG/CSS and
  browser accessibility mechanisms;
- a native mask implements the same front directly with its own bounded
  layout and raster work;
- a deterministic linear mask consumes the same value without claiming
  two-dimensional geometry;
- a constrained mask may expand an ordinary canonical plot back and realize
  admitted layout, composition, and graphics operations recursively.

The linear path is a complete nonvisual realization, not a fake framebuffer.
Direct masks do not advertise lower layers they do not implement.

## Ordinary recursive realization

A recursive mask uses the ordinary Conduit path:

```text
canonical plot/back expansion
-> checking
-> exact planning
-> lowering and preparation
-> production conduit-kernel execution
-> admitted Host Calls and resources
-> bounded signs
```

There is no mask scheduler, private recursive executor, or second semantic
graph. The exact expanded plot and plan record every selected back, leaf
implementation, host, boot, operation, and resource. Direct and recursive plans
must differ because their realization differs; the presented user plot and its
Face meaning does not change.

The mechanically generated `cargo xtask check catalog matrix` report is the static
coverage view. It distinguishes a direct browser implementation from an
installed constrained recursive realization and does not promote installed
coverage into a claim about a current boot.

## Admission below the waist

A drawing or layout operation becomes portable meaning only when two materially
different masks can implement its exact bounded contract without sharing
toolkit internals. This admits reusable geometry, clipping, composition, text,
and icon intent when they have independent semantic value. Raster helpers,
glyph caches, callbacks, DOM identifiers, widget objects, framebuffer addresses,
and private line/path helpers stay inside their mask realization.

The default Patchbay canvas presents the user's program. Recursive realization
is available for explicit inspection; it is not injected into the maker-facing
graph merely because Patchbay can inspect its own plan.

## Interaction and failure

Meaningful input returns through the same semantic interaction boundary as
Face output. Pointer, keyboard, touch, hit testing, and focus are local
mechanisms; they produce the bounded interaction requests owned by #694. A
mask back does not gain edit authority from owning geometry.

Mask failures remain realization facts. A lost browser document, native
surface, display resource, font/icon implementation, or recursive leaf cannot
rewrite the Face or the user's plot. Replanning or fallback is permitted
only through ordinary plan rules and exact current offers. A still-available
linear mask remains a separate truthful realization, not evidence that a
failed graphical realization succeeded.

## Conformance proof

Cross-mask conformance compares the exact Face basis and normalized
interaction context, subjects, roles, relationships, properties, typed content,
wording, actions, inputs, projection/navigation, disclosure, semantic order,
time, and provenance. It deliberately does not compare pixels, coordinates,
typography, wrapping, toolkit nodes, prose, or screenshots.

The conformance fixture feeds one bounded specimen containing unfamiliar
domain roles and relationships, typed content, semantic order, actions, inputs,
and multiple disclosure depths to graphical, deterministic-linear, and
generative masks. It must preserve the same semantic identities without
application-specific rewiring. Separate plan proof establishes that direct and
recursive masks select distinct exact realizations without changing the source
Face. Current tests may name the serialized Rust value `Presentation`; that is
an implementation migration marker, not architectural vocabulary.

The [grammar conformance method](architecture/presentation-grammar-conformance.md)
pressure-tests that claim against heterogeneous encounters. It distinguishes a
missing cross-medium human meaning from familiar interface furniture and makes
each proposed extension prove its place at the semantic waist.
