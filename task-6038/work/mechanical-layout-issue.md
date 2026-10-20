Parents/contracts: #5365 checked structural component recipes, #5357 native layout, #5364 structure/Look separation, #5372 correspondence. User expanded the 2026-10-20 request beyond pagination to mechanical layout writ large: rows, columns, grids, forms, alignment, resizing and overflow. #6158 is one concrete regression case, not the whole feature.

## Outcome

Provide reusable, deterministic, bounded layout composition below Face. Checked component structure expresses how projected meanings compose; the selected Mask measures and arranges it for actual viewport/font/accessibility constraints. Reuse ordinary Plot/Fore/component machinery; no parallel widget ontology, component language, runtime or application truth.

Haiku provides useful precedent: nested BLayout/BLayoutItem composition; group/grid layouts; minimum, preferred, maximum size and alignment; height-for-width; layout invalidation. Qt similarly nests box/grid/form layouts and uses size policies. These are mechanical negotiations over explicit structure, rather than coordinates or text-specific layout guesses.

Primary references: [Haiku layout introduction](https://www.haiku-os.org/docs/api/layout_intro.html), [Haiku LayoutItem contract](https://github.com/haiku/haiku/blob/master/headers/os/interface/LayoutItem.h), [Qt layout management](https://doc.qt.io/qt-6/layout.html).

## Required behavior

- Reusable nested row/column groups, grids with bounded tracks/spans, form label/field associations, alignment, padding/gaps and finite flexible space. These compose across unrelated components rather than matching Crèche names or labels.
- Bottom-up measurement using actual selected font/text/accessibility metrics; explicit minimum/preferred/maximum extents and wrapping height for the chosen width. Top-down arrangement allocates available space according to bounded grow/shrink rules.
- Responsive alternatives selected from measured feasibility and checked structure. Collapse a grid/columns coherently; do not rely only on one magic screen-width cutoff or fill a sequential flat list until it overflows.
- Structural groups carry explicit break/continuation policy. Associated help/label/action/current value stay together when feasible. Oversized groups split through declared accessible continuation, without dropping content, exceeding limits or looping to find a layout.
- Distinguish geometric shortage from row/command/text/surface/resource shortage. A sparse page can be necessary under a finite budget; expose that disposition and choose a deterministic legal arrangement. Preserve the existing 48-command/192-byte text/20-row/eight-surface bounds unless separately reviewed finite admission proves an alternative.
- Source reading/action order remains authoritative. Visual placement is not a new action order or selection identity. Keyboard/pointer/accessibility lowering retain the same checked Face bindings and current acknowledged Show.
- Font/Look/viewport/content/accessibility changes invalidate affected measurement and produce a newly admitted layout/Show before interaction uses new hit regions.
- Browser and native may realize the same structural intent differently. Linear/spoken access preserves meaning without needing a grid. Unsupported constraints must fall back/refuse explicitly.

## Delivery and acceptance

1. Audit current checked recipe/admission and native/browser lowering; identify existing reusable responsibilities before extending contracts. Keep exact structure identity distinct from measured realization and Look.
2. Establish a small finite measurement/arrangement contract and explicit bounds: node/depth/track counts, arithmetic, candidate passes, scratch/retained storage and work. Prepare before Play; no hidden unbounded solver, allocations, retries or caches.
3. Prove the same primitives on at least two unrelated real component compositions (e.g. Crèche and Todo/Patchbay), including ordinary/nested columns and a genuine grid/form. Preserve generic unknown-component fallback.
4. Regress #6158 plus narrow/wide, translated/long Unicode, large text, minimum-size conflicts, zero/one/many children, spans, unavailable controls, invalid cycles/constraints, deliberate overflow and repeatable layout. Check complete meaning, non-overlap/visibility/hit regions/order rather than screenshots alone.
5. Retain actual normal native QEMU and pinned one-worker/zero-retry browser proof where the selected Back supports the contract. Record capability gaps and exact CI/integration/publication boundaries.

Coordinate with #5994/#6038 and #5520: one active native scene implementation; no unrelated rewrite of the current panels PR. Deliver reviewable seams/slices, not a bundled universal toolkit. Existing issue text that limits the old quick win to installed-path selection is not a reason to disregard this newly authorized broader request; preserve the old history and state which later slice owns each extension.
