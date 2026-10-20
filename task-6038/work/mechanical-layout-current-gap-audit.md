# Mechanical layout: current reusable machinery and gaps

Read-only audit of e29922753. Live #5357/#5365/#6158 reread; no Source edits, builds/tests, guest, fetch/Actions or PR work. User's broadened outcome is reusable deterministic layout (rows, columns, grids and nested composition), not only another Crèche pagination correction. No new widget ontology, second layout DSL or learned layout policy is warranted.

## What exists

- `semantics/face/types.conduit:708-731`: checked structural recipe v1 binds Subject/Bound/ActionTarget/Role selectors to at most8regions, group IDs0..7, up to20subjects/region,1–2columns, gutter0..24 and one reflow threshold. This is ordinary typed Fore/configuration data.
- `semantics/face/src/component_recipe.rs:35-94`: exact current Face binding, required Bound-slot checks, duplicate/overlap/capacity refusal and complete canonical reading. `:127-174` admits simple content/column widths with checked arithmetic. Required semantic bindings and fallback already have a home; no duplicate semantic layer is needed.
- `targets/conduitos/src/native_face_scene/layout.rs:145-246,298-318`: actual font/scale-specific text measurement, UTF8-safe ≤192B chunks, bounded row geometry,20rows/48commands and explicit pages. Measure uses the same typography cursor as paint. This is genuinely reusable mechanical leaf layout.
- `layout/panels.rs:1-170`: independent y lanes for first authored pair of groups, equal column widths and full-width later groups. `panels/ordering.rs:4-48` admits input work, assigns authored-region geometry order and restores original Source reading/control order with a bounded permutation. There are no Crèche domain IDs in this production layout, but its policy is deliberately narrow, not a general nested container system.
- `native_face_scene.rs:158-221` prepares a whole configured scene from exact Face+recipe+Look expression. Fallback, source reading, current action references and fresh Show remain separate from geometry.
- `targets/browser/runtime/src/owner_face_mask/component.rs:18-27,69-100` exports checked regions/groups/columns/gutter/fallback from the selected current configuration. `targets/browser/handbook/owner-face-component.mjs:115-145` uses ResizeObserver and actual width to choose1/2columns and tag bound regions. `owner-face-look.css` already uses real CSS Grid/minmax. This is a valid browser realization, but CSS's richer intrinsic layout has not become portable checked recipe meaning; it does not make native a grid engine.

#5357 explicitly requires reusable bounded size constraints/nonoverlap below Face; #5365 explicitly already describes min/preferred/max, composition trees, overflow/flow, nesting, structural variants and finite depth/cycle checks. These are existing architectural intent, not justification for a new high-level markup language. The old #5990 instruction “not a new grid system” bounded that quick-win slice; it did not complete or forbid the user's now-explicit broader general layout objective.

## Actual gaps and contract ownership

| Concern | Current capability | Missing/general seam |
|---|---|---|
| Measure→arrange | Text measured at an already assigned width; panels choose equal widths first. | A leaf demand/result with min/preferred/max extents, baseline/alignment where supported, and height-for-width. Parent containers must aggregate child demands before assigning rectangles. Reuse font metrics, do not guess from labels. |
| Rows/columns/nesting | Flat regions + two lanes; later groups span below. | Finite nested group composition and explicit main/cross-axis alignment. A row sums main-axis demand and takes cross-axis maxima; a column does the converse. Mechanical distribution between min/preferred/max must be deterministic and bounded. |
| Grids | Browser CSS has tracks; native has no general grid representation. | Finite explicit rows/columns, child placement/spans and track constraints, with checked indices and deterministic track distribution. Start with a bounded subset; do not implement a browser CSS engine or unconstrained solver. |
| Keep/group/break | Group number gives visual association; flat `Item` admission measures one text chunk at a time. | Measure an intended atomic field/component cluster as a unit before breaking. Explicit behavior when a complete group cannot fit is necessary: next column/page, selected collapse, permitted internal break with continuation, or typed refusal. Existing group labels do not currently promise keep-together. |
| Responsive alternatives | One scalar threshold switches max columns to1. | Ordered finite structural alternatives checked for the same required slots/actions/reading order. Choose the first admitted fit under viewport, metrics and resources; record chosen variant/refusal reason. No hidden removal of required content. |
| Invalidation | Fresh Face/configuration builds a new whole scene; browser observer reflows width. | A complete explicit measurement key (Face revision/content, recipe/variant, Look/scale, Host metric-resource identity/generation, viewport/constraints). Bounded dirty propagation or whole-tree rebuild is valid initially. Never repaint with changed fonts against stale rectangles/hits. |
| Resource admission | Strong finite scene/row/text/surface limits and bounded ordering scratch. | Admit container nodes/depth/tracks/spans/measure calls/scratch plus final rendering costs before Play. Geometry fit and48command fit are independent. Keep peak simultaneous preparation storage and frame work visible. |

## Existing data versus reviewed extension

Can reuse **without a new public contract**: exact semantic selectors/Bound slots; current region membership and authored region order; complete fallback/reading; existing1/2columns/gutter/reflow; Host font metrics; renderer-private measured leaf records and deterministic container algorithms. A private keep-together improvement for rows that belong to the same exact source field can be a coherent existing-policy correction if its oversized-group fallback is explicit. It must not silently redefine every group ID as unbreakable.

Needs an **explicit reviewed versioned extension of the existing recipe Fore data**: author-requested min/preferred/max, alignment, nested container topology, row/column/grid choice, tracks/spans, authorial keep/break policy and responsive variants. v1 cannot truthfully carry these today. Do not encode them into region identity strings, generic Face properties, colors, action names, `group` integer meanings or browser-only CSS. Reuse ordinary typed records/sums/sequences and existing Plot/Kind/Fore authoring; new parser syntax is unnecessary. Review portable size/unit semantics and Host translation explicitly rather than making DOM pixel conventions authoritative. A bounded compiled layout representation below that checked contract may differ per Host.

## Responsibility seams

1. **Structural admission (Face-adjacent recipe responsibility):** input checked recipe+Face; output bound finite component/container graph and canonical semantic/control ordering, required content and admissible variants; validate topology/cycles/depth/cardinality/indices/size contradictions. No font files, viewport device facts or painting.
2. **Host measurement:** input bound leaves + selected font/Look metrics + finite width constraints; output intrinsic demand and height-for-width results/cost. No action creation or owner mutation. Use one cache key or no cache; a bounded whole-tree prepare is simpler than speculative incremental machinery initially.
3. **Mechanical arrangement:** input demands+finite container policy+viewport/resource envelope; output rectangles, baseline/alignment, variant/break/continuation decisions and resource accounting. Keep semantic/control order independently recoverable from visual placement. A pure deterministic module can support native and correspondence fixtures; browser may lower equivalent constraints to its own admitted CSS primitives.
4. **Graphics/hit lowering:** existing native scene/frame and browser control lowering consume exact arranged bindings. Final surface/command admission and acknowledged Show stay here; neither arranger nor CSS becomes a planner or second source of interaction authority.

## Bounded reviewable delivery

- **A — reuse/measurement seam:** establish explicit leaf-demand and container measure/arrange interfaces with existing metric fixtures, no schema or behavior redesign. Preserve old public paths by narrow adapters; owning package checks, not unrelated workspace reruns by default.
- **B — atomic field + deterministic row/column:** reuse current bound subject/region references to measure complete field clusters, account chrome/control/text commands before placing them, and arrange admitted rows/columns. Use #6158's prompt/action/current-value orphaning and sparse-page report as a retained real regression, alongside unrelated component fixtures. This must improve placement, not just move every orphan to another mostly empty page. Identify exact Source/recipe/viewport when reproducing the user attachments; their desktop image dimensions are not guest dimensions.
- **C — checked reusable topology/constraints:** add the smallest versioned existing-Fore extension with bounded nested Row/Column and min/preferred/max/alignment/keep/break data, positive and negative conformance, same semantic bindings under responsive alternatives. Avoid an omnibus all-grid/all-style PR.
- **D — finite grid + both Host lowerings:** add reviewed bounded tracks/spans over the same component graph; deterministic arrangement and browser lowering prove matching required content/control order/refusal, not identical pixels. Include genuinely different structures/data, not only the Crèche recipe.
- **E — integration/invalidation:** exact current normal product proofs with resize/scale/font resource changes, fresh layout/Show and unchanged Source. Reuse #6158 before/after, long Unicode/unbreakable text, empty/singleton/many children, too-small extents, contradictory sizes, overflow, stale resource generations and command pressure. Preserve48commands/192Btext/20rows/8surfaces; explicit multi-scene/surface admission requires its own budget, not a quiet raise.

This is a deterministic layout pipeline: measure children, propagate constraints, select a finite fit, arrange and admit exact costs. It can be mechanically sophisticated without heuristics tied to one application or an LLM in the runtime. It does not require adding semantic widgets, duplicating Source state, a second Look engine, ambient fonts, unbounded invalidation or a universal optimization solver.

## Essential small core for #6159

The useful innovation is making **fit a checked result**, not building a framework. Keep four small values: a bound child reference; measured demand; arranged rectangle/continuation; admitted cost. A form is a composition of existing bound label/field/action/validation meaning, not a new authoritative semantic widget. Initially omit animation, arbitrary constraints, runtime code callbacks, general caches, custom solvers and decorative style systems.

A finite candidate pass can be explicit: (1) admit graph and candidate count; (2) measure intrinsic widths using exact selected metrics; (3) assign row/column/grid widths between admitted minima and maxima with deterministic tie-breaking; (4) measure height-for-width once at those actual widths; (5) arrange vertically and check whole-group breaks plus command/surface costs; (6) accept first valid authored candidate, otherwise use explicit fallback/refusal. No free fixed-point loop. If a policy needs width reconsideration after height results, admit a finite second candidate rather than iterating until it happens to stabilize. Bound measure calls by candidates×nodes×declared width samples; memoization is optional and separately finite.

Invariants worth defending:

- Geometry may reorder placement; Source reading/control identity and required meaning remain recoverable and unchanged.
- Measured width/height and paint use identical metric resource, role and scale; a changed identity invalidates the arrangement.
- Minimum≤preferred≤maximum is checked; unsatisfiable constraints produce typed alternatives/refusal, not clipped truth.
- Group cohesion is evaluated before fragmentation; an overlarge group follows its admitted break/continuation policy.
- A geometry fit that exceeds commands/bytes/work/storage is not a fit.
- The selected candidate is explicit, reproducible and associated with the prepared Show; no invisible Host retry/policy can change actions or omit mandatory content.

These invariants unify the general goal with #6158 without reducing it to pagination. The row/column core should solve more than one real structure before a grid extension is declared reusable. #6159 owns the broader reviewed contract; #5994 serial order and current #6055 admission are not bypassed by this preparation.
