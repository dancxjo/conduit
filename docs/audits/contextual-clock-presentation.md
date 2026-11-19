# Contextual clock presentation: existing mechanisms and first integration cut

Review of the user-supplied **Contextual Looks and Subject-Centered
Presentation** proposal, 17 November 2026. This is a design review and development
slice, not a new canonical Look definition or a deployment receipt.
The starting development source is `fe1b49b37e`.

## Recommendation

Choose **B: contextual selection over existing checked components and Looks**.
Keep `LookInfo` as finite expression policy. An application supplies ordinary
checked component Plots, their literal semantic bindings and layout alternatives,
and compatible Looks. Context selection chooses among those documents/roots;
it cannot confer Host capabilities, output authority or domain operations.

The [Face/component/Look contract](../architecture/face-component-look.md) remains
in force. The broader proposal's use of “Look” for a package containing both
composition and expression is useful product vocabulary, but is not adopted as
an expansion of the `LookInfo` wire contract here.

## Existing mechanism map

| Responsibility | Existing producer/consumer | What it establishes |
| --- | --- | --- |
| Semantic encounter | `Face`, `FaceSubject`, `FaceRole`, `FaceBasis` | Immutable bounded graph, typed facts/actions and exact encounter provenance; not an enduring identity issuer |
| Required semantic slots | `ComponentSubjectSelector::Bound`, `component_slots::validate_binding` | Exact Subject, semantic role, property kinds and original action contracts |
| Composition | `ComponentRecipeInfo`, `ComponentLayoutInfo`, ordinary component Plot | Bounded regions, layout candidates and explicit complete generic fallback |
| Expression | `LookInfo`, `admit_visual_look`, `VisualUserConstraints` | Independent bounded appearance preferences, user/Host override and explicit fallback |
| Source/Plan correspondence | `ConfiguredMaskConfiguration`, `prepare_layout_component_document` | Original literal recipe/layout and selected compatible Back/placement; not a memory reservation |
| Native realization | `NativeFaceScene::prepare_structured_look` | Measured bounded native layout over original Face references; CPU construction is not physical output |
| Replacement | Existing owner Mask route/Show retirement and encounter rules | Separate realization identity, stale-return refusal and constrained focus continuity; not application restart |
| Small display | Existing `PreparedDisplayMask::prepare_show` | Owned bounded display material without a continuing full Face borrow; not complete physical lifecycle acceptance |

Source references:
[semantic slots](../../semantics/face/src/component_slots.rs),
[configured gate](../../semantics/face/src/configured_mask.rs),
[native preparation](../../targets/conduitos/mask-offer/src/layout_configured.rs),
[owner route](../../products/conduit/src/body_owner/native_mask_route.rs), and
[seven-segment boundary](../../bodies/environment/docs/display-mask.md).

## First selector and implementation contract

The first cut is deliberately closed: an explicit encounter is `tile` or
`fullscreen`. `AmbientClockEncounter` maps those two disjoint values to exact
reviewed Source roots. Unknown values return `UnsupportedEncounter`; no casing,
substring, declaration-order, predicate-count or CSS-cascade rule supplies a
match. Ambiguous candidate sets cannot be expressed in this first profile.
A general predicate/dominance engine is not implemented.

- `tile` selects `ambient-clock-tile`: one required clock region, then complete
  generic fallback for weather, images, other meaning and actions.
- `fullscreen` selects `ambient-clock-mask`: explicit clock, weather and image
  regions, then the same complete fallback obligation.
- `ambient-clock-mask-compact` independently exercises another Look over the
  fullscreen component. Density changes alone are not called a new composition.

The [authored Source](../../semantics/face/components/ambient-clock-mask.conduit)
uses current Conduitese, with the ordinary Fore:

```text
plot ambient-clock-component (
    look: LookInfo
    >> face: Face
    show: Show >>
    interaction: FaceInteraction...| >>
)
```

The component invokes `face/component-mask` with literal `recipe` and `layout`,
forwarding only `look`. No new `look { when ... }` syntax, `FaceSlice`, renderer
callback, component executor or application registry is introduced.

`ambient_clock_mask_source` binds three explicit original Subject references into
that reviewed literal Source. References are distinct, nonempty and at most
256 bytes. Quoting and simultaneous substitution prevent a supplied reference
from becoming syntax or another binding. The resulting bytes establish a Source identity. Checked and expanded identities
are established only by successful ordinary compilation; planning and exact
bound-slot validation against the actual Face remain required. Source generation alone
is not checking, authority, a reservation or a Show.

The selection function is context policy only. The consuming owner must obtain
actual current Host/Mask offers and measured surface facts independently, enforce
user accessibility constraints, and admit the selected Plan/resources. A string
such as `gui` cannot establish possession. This cut does not automatically infer
an encounter from width or authorize a root because its name matches.

### Rule if contextual matching is extended

The recommended next contract is a finite source-authored candidate set over
ordinary checked Info/choice Types, not the proposal's illustrative CSS-like
syntax. Each predicate names an independently owned fact and an exact operation:
identity/contract equality, encounter equality, or a bounded surface interval.
Absent or unknown facts do not count as true. Evaluate compatibility and hard
user/Host constraints independently; a selector match is only descriptive.

For matching candidates, specificity is implication under those declared fact
schemas: A dominates B only when A's predicates imply B's and not conversely.
Interval containment and exact categorical predicates can support this finite
comparison; arbitrary graph predicates or counting clauses cannot. More than one
undominated candidate, including equivalent selectors with different targets,
returns `Ambiguous`. No match requests an independently admitted generic
fallback. Unknown selector versions return `Unsupported`; resource refusal stays
`ResourceRefused`, not `NoMatch`. Every outcome retains candidate/Source identity,
matched fact provenance, selected Plot/Fore and the separate admission outcome.
This rule is a recommendation, not an implemented general selector engine.

## Identity and provenance

Both compositions consume the same original Face and Subject references. Context
selection must not edit Face facts/actions, replace its basis, or restart its
application Play. Selected root, checked Source, recipe, Look, actual Back/Plan,
constraint outcome and acknowledged Show are separate provenance.

There is a real enduring-identity gap in the deployed ambient producer:
`Builder::new` hashes `FaceContributionBasis`, including Plan/active Play, into
its Subject prefix. Keeping an exact Subject while switching presentation is
possible; claiming that ID survives another producer Plan/Play or producer restart is not.
Showing the same unchanged Face on another Mask Host can retain its Subject IDs.
The first cut must preserve current references, not mint a substitute enduring
ID in the selector. Stable Subject identity requires its owning domain contract
and migration; coordination with that owner remains an explicit integration task.
A rooted encounter is not a second sovereign Body Face or a stored Face per thing.
Vector similarity and embeddings remain derived discovery information and never
establish an exact selector match, truth or authority.

## Fallback, time and lifecycle rules

1. Bound-slot mismatch or layout/offer/resource failure remains an explicit
   preparation refusal. Where existing policy permits, a separately admitted
   generic Mask presents the original meaning. Generic content is not deleted
   to make a specialized candidate fit.
2. User constraints and hard Host limits outrank the requested Look. Inability
   to produce readable, reachable content refuses; it does not shrink text below
   the required scale or acknowledge an unusable Show.
3. Context changes use existing Mask replacement: prepare the replacement,
   retire/reseal the old Show as the owning protocol requires, acknowledge the
   actual new occurrence, and reject old interactions. CPU scene equality does
   not prove that transaction or physical acknowledgement.
4. Page/frame progression is distinct from observations. A local page change
   cannot refresh a measurement. A clock extrapolating displayed civil time needs
   an admitted clock basis, validity/timezone rules and deadlines; the current
   deployed one-second observation cadence is not proof of such an optimization.
5. Media demand, pressure, buffer custody and deadlines remain ordinary admitted
   execution responsibilities. No Look-private scheduler is added.
6. Generated Source, compiler scratch, prepared configuration, retained Face,
   layout, replacement overlap and pending acknowledgements all require their
   original resource custody. External preparation is not external authority.

## Genuine gaps and testable milestones

1. **Checked CPU reference (this coding cut):** two real literal component
   compositions, closed encounter selection, Source specialization and existing
   native preparation. Test both Looks, same Face/basis/Subject/actions, unknown
   meaning, unavailable weather, escaped bindings, missing/wrong slots, small
   geometry and accessibility. Claim only tests actually executed.
2. **Installed native consumer:** join these roots to the actual observed layout
   renderer and sealed owner route. Current owner/native selection is
   Todo-specific. Reuse the previously developed photograph resource/pixel path;
   this starting dev source has the semantic image type but not that consumer.
   Prove tile/fullscreen replacement with unchanged application Play, retirement,
   stale-return refusal and actual scanout on motherbrain/threebee.
3. **Context provenance:** extend only the needed existing owner configuration
   with independently owned encounter/surface/preference facts and explicit
   diagnostic/fallback outcomes. Review any further selector schema or syntax
   separately; general graph predicates, CSS matching and plugin loading are not
   prerequisites.
4. **Enduring Subject IDs:** coordinate domain ownership/migration and prove IDs
   across context, observation revision and the independently required restart
   boundary. Do not infer this from a fixed fixture Subject string.
5. **MCU lifecycle:** reuse the existing seven-segment path with one complete
   lifetime trace through preparation/admission/Face/Mask/physical output and
   continuing execution. Membership/history/restart remain separate acceptance
   claims. A small prepared frame does not prove whole-device RAM/stack/timing.
6. **Other modalities:** browser replacement, seven-segment contextual selection
   and an agent-facing artifact are subsequent independent proving cases.
   Instructions or a skill manifest never grant an operation.

Existing ownership remains with [#5364](https://github.com/dancxjo/conduit/issues/5364)
(separation), [#5365](https://github.com/dancxjo/conduit/issues/5365)
(checked component Fores), [#5378](https://github.com/dancxjo/conduit/issues/5378)
(ordinary composition), [#5368](https://github.com/dancxjo/conduit/issues/5368)
(multimodal Look), [#5377](https://github.com/dancxjo/conduit/issues/5377)
(authored syntax), and [#6190](https://github.com/dancxjo/conduit/issues/6190)
(installed native acceptance). This review does not close those issues or create
another styling/Subject Graph/runtime architecture.
