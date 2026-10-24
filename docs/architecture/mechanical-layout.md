# Mechanical layout in a Mask

**Status:** design proposal for [#6159](https://github.com/dancxjo/conduit/issues/6159).
This document does not claim an implemented layout contract or accepted runtime
capability. [#6158](https://github.com/dancxjo/conduit/issues/6158) is one concrete
regression; the outcome includes rows, columns, grids, forms and nested composition.

A component describes spatial relationships and needs. A Mask finds an admitted
realization. A Look gives that realization expression. Positions follow from
these inputs; they are not application facts.

## Preserve the ontology

The [canon](../conduit-canon.md#execution-and-face) remains authoritative.

| Identity | Responsibility |
| --- | --- |
| Face | Exact source facts, typed content, offered actions, semantic association/composition and explicit semantic order. |
| Checked component Plot/Fore and structural recipe | Bind existing meanings and actions; compose their presentation through finite slots, containers and admissible structural alternatives. |
| Look | Expression and appearance, including supported typography, density and spacing vocabulary. It cannot change source facts, action authority or required component bindings. |
| Selected Host Back and resources | Exact implementation, measurement resources, supported text behavior, viewport and finite resource offers. These are not facts embedded in authored application Plots. |
| Mask | The ordinary planned Plot realizing an exact Face revision through those bindings, expression and admitted resources. |
| Show | The finite acknowledged realization, including exact current interaction bindings. It is distinct from source, recipe, measured geometry and runtime Plan. |

Semantic association may already belong to Face: these meanings are grouped,
contrasted or subordinate. The Mask decides how that association becomes spatial.
A row, grid track, rectangle or font file does not become a Face subject type.
Components remain ordinary checked compositions, not another widget ontology,
parser language or executor.

Explicit semantic order and checked traversal commitments survive rearrangement.
Serialization order alone is not semantic order. An unordered Face permits a
Mask to choose a navigable encounter without manufacturing a new source ordering.

## Compose first, measure, then arrange

The reusable core has four results: a bound child reference, measured demand,
arranged geometry/continuation, and admitted cost. None duplicates Body state.

1. **Bind and validate.** Resolve the checked recipe against the exact Face.
   Validate required slots, action/argument targets, finite topology and variants.
   Reject cycles, invalid indices and contradictory constraints before effects.
   Admit the measurement work bound and reserve its graph, scratch and result
   storage before running measurement. Final output admission is a separate check.
2. **Measure demands.** Resolve intrinsic minima, preferred extents, maxima and
   supported baselines using exact selected text/Look metrics. Aggregate children
   through containers without inventing meaning from their displayed strings.
3. **Assign inline space.** Distribute available space between admitted minima
   and maxima using explicit grow/shrink rules. Apply a documented deterministic
   rounding rule. Spare pixels do not disappear through accumulated truncation.
4. **Measure at assigned widths.** Obtain wrapping and block demand at the actual
   widths that will be painted. Measurement and painting share resource identity,
   role, scale and supported text-breaking behavior.
5. **Arrange and fragment.** Allocate block space, align baselines/edges, apply
   declared keep/break rules and retain explicit continuation.
6. **Admit the complete result.** Check actual geometry, visibility, commands,
   bytes, surfaces and costs against the already admitted work/storage envelope.
   Prepare a Show with exact hits;
   publish/acknowledge through the existing Mask route.

This is a finite pass over declared alternatives, not a convergence loop.
The checked recipe declares candidate priority; preparation selects the first
feasible candidate in that order, with documented deterministic tie-breaking.
If height results require a different width policy, that is another explicitly
bounded candidate. No hidden retry searches indefinitely for a pleasing result.

## A small compositional vocabulary

Rows and columns aggregate demand along their main and cross axes. They support
explicit alignment, insets, gaps and bounded flexible space. A form composes
bound labels, inputs, actions and validation through these relationships; it
does not create a new semantic Field or Button Kind.

Structural constraints define topology, hard extents and required keep/break
rules. Look resolves typed expression hooks for preferred spacing and density
within those constraints; it cannot rewrite the topology or candidate set.
Specify precedence when a preferred Look gap conflicts with a hard constraint.

Grids add finite tracks, placements and spans. Track sizing must account for
children's intrinsic contributions before distributing remaining space. Shared
alignment between nested containers needs an explicit bounded relationship;
it must not emerge from unrelated containers guessing matching coordinates.
Track/span arithmetic and allocation passes have explicit bounds.

Structural alternatives express different arrangements of the same required
meaning. A multi-column composition can collapse coherently when its measured
minima no longer fit. Font changes, localization and large text therefore affect
feasibility, rather than relying entirely on a fixed screen-width breakpoint.

Every alternative preserves its required binding set and explicit semantic order.
An alternative may not obtain a fit by quietly removing an unavailable reason,
an offered action or a required value. Optional disclosure follows existing
Face/component laws and remains accessible through its admitted path.

## Fit includes the cost of showing it

Space shortage and resource shortage are different outcomes. An arrangement can
fit geometrically and still exceed its admitted command or storage budget.
The current native 48-command, 192-byte command-text, 20-row and eight-surface
limits remain unchanged by this proposal. More pixels alone do not admit more
commands. Multiple scenes/surfaces require explicit admission, not a quiet raise.

Each atomic group has an explicit fragmentation policy. A label, action and
current value should remain together when their checked component requires it.
If the group cannot fit whole, the selected policy may move it, select another
composition, allow a defined internal break with continuation, or refuse/fall
back. An impossible keep request cannot cause clipping or an endless loop.
Headings must not be stranded from their required following content.

A page with unused space can be correct under a hard limit. The prepared result
should retain the reason for a break or collapse for inspection and proof.
The encounter should communicate useful continuation, not implementation trivia.
Readable fallback and pressure/unsupported/refused outcomes remain distinct.

## Lessons from existing systems

[Haiku's Layout API](https://www.haiku-os.org/docs/api/layout_intro.html) composes
groups and grids through nested layout items. Its
[item contract](https://github.com/haiku/haiku/blob/master/headers/os/interface/LayoutItem.h)
includes minimum/preferred/maximum size, alignment, height-for-width and
invalidation. [Qt's layout management](https://doc.qt.io/qt-6/layout.html) similarly
separates box/grid/form geometry management from application behavior.

CSS contributes more than a collection of properties:

- [Intrinsic sizing](https://www.w3.org/TR/css-sizing-3/) distinguishes content
  demands from externally assigned space. Automatic minima must be explicit and
  testable; a flexible track does not automatically mean its content can shrink.
- [Grid](https://www.w3.org/TR/css-grid-2/) demonstrates intrinsic contributions,
  bounded track allocation, spans and shared nested alignment. Its accessibility
  rules also distinguish visual placement from speech and navigation order.
- [Fragmentation](https://www.w3.org/TR/css-break-3/) treats keep/break decisions,
  oversized content and continuation as layout responsibilities.
- Logical inline/block directions, a coherent box model, baseline alignment,
  explicit overflow and invalidation are fundamental behavior, not polish.

Borrow these lessons and applicable conformance cases with provenance. Browser
CSS remains a valid selected Back implementation. This proposal does not require
CSS parsing in the kernel or claim CSS conformance. Its initial box rules should
be explicit, without implicit margin collapse or unexplained inherited geometry.

## Finite realization and stable reflow

An executable profile must declare maxima for nodes, depth, tracks, spans,
alternatives, measurement samples, arithmetic, work and simultaneous storage.
For candidate count V, nodes N and an admitted query budget S per node/candidate,
measurement calls have a declared bound no greater than V × N × S. S counts all
intrinsic, basis, height-for-width and repeated grid requests; bounding only
distinct widths is insufficient. Track/span work needs its own
bound; this expression is not a complete algorithm or whole-scene resource bound.
The first executable consumer must publish concrete limits and peak measurements.

Preparation may reserve finite storage according to its Host profile. Play must
not discover growing caches, allocate an atlas, add undeclared alternatives or
retry a failed Host effect. A bounded whole-tree rebuild is an acceptable first
implementation; incremental invalidation is optional, not a prerequisite.

The measurement identity includes exact bound content/revision, structure and
candidate, resolved Look/scale, metric resource identity/generation, viewport
and user constraints. Changes invalidate affected geometry and hits. Repaint
cannot substitute a new font against rectangles measured with an old one.

Retain focus by stable checked binding when it remains eligible, then apply an
explicit fallback when it disappears. Page, draft and focus remain bounded
Mask-local state. Reflow alone cannot commit a source edit. An interaction still
returns through the existing current Face/Show validation and source-owned result.

## Decisions required before extending the public Fore

The current recipe has flat regions and limited column geometry. It cannot
already express this proposal. Review a versioned extension of its ordinary
typed Fore/configuration, including selected Kind/Back compatibility and exact
identity changes. Do not overload identity strings or group integers with a
hidden new grammar, or keep a public compatibility executor.
Supported versioned Backs may coexist under exact eligibility; an older Back
must reject unsupported versions or constraints rather than silently ignore
them. Coexistence does not introduce another executor. A recipe's child sequence
has its declared structural/traversal meaning, not automatic Face semantic order.

Decide portable size/unit rules explicitly. Intrinsic requests, proportions and
Look-relative requests need a defined resolution dependency; DOM pixels must not
become the portable authority. Host-local coordinates are resolved realization
data. Percentage/intrinsic cycles must be excluded or resolved by an explicit
finite rule. Physical quantities require actual supported Host correspondence.

Specify writing-direction and text-breaking support per realization. A logical
axis model does not prove native bidi, vertical text, grapheme shaping or IME.
Unsupported behavior remains a typed refusal or honest fallback, not invented
coverage. Preferred sizes and soft keep requests need explicit precedence when
they conflict with hard limits or required access.

## Delivery and proof

Establish one responsibility seam at a time: existing measurement interfaces;
coherent bound groups and deterministic row/column distribution; reviewed nested
constraints/alternatives; then finite grids and both Host lowerings. Preserve
the current serial native ownership and admit small reviewable PRs.

Use at least two unrelated real component compositions, not only a Crèche fixture.
Test empty/singleton/many children, nested groups, spanning tracks, non-divisible
widths, long/unbreakable Unicode, changed fonts/scale/direction, conflicting
minima, command pressure and oversized groups. Check complete meaning, required
access, non-overlap, traversal, current hits and zero source effects during reflow.
Browser and native may produce different legal geometry; linear/spoken meaning
must remain independently reconstructible.

Retain deterministic conformance and exact-source normal native QEMU/browser
proof separately. The user screenshots for #6158 have no supplied Source/image
identity; their desktop pixel dimensions alone do not establish guest mode.
Preserve that uncertainty alongside current reproducible cases. Design review,
compilation, emulator execution, physical use and stable publication prove
different things. This proposal closes none of those implementation gates.
