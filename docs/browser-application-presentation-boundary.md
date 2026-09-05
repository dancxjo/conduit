# Browser application presentation boundary

**Status:** Phase 0 inventory and migration boundary for [issue #2050](https://github.com/dancxjo/conduit/issues/2050)

**Observed base:** `b0f0e54c6900ec1459abf3d3a451d51a8b3fe0bf`

**Scope:** Book, Crèche, Patchbay HTML, and the browser Host presentation mechanism

This document records the boundary before code is moved. It is not evidence that
the shared runtime or any application migration exists. Current executable truth
continues to belong in [`STATUS.md`](../STATUS.md).

Current source ownership is described in [Browser product source ownership](browser-product-source-ownership.md). The inventory below preserves its observed base.

## Decision

Book, Crèche, and Patchbay are applications. They own their meaning, application
state, actions, and renderer-neutral presentation descriptions. The browser Host
owns the browser presentation mechanism: bounded description validation, DOM
creation and updates, event capture, and delivery of finite application events.
Patchbay's native renderer remains an independent renderer of the same
renderer-neutral Patchbay state.

The migration must end with this dependency direction:

```text
Book model ---------\
Crèche model --------> finite presentation description + event protocol
Patchbay model -----/                         |
                                               v
                                  browser Host DOM renderer
                                               |
                                               v
                                       browser DOM/events

Patchbay model ---------------------> native Patchbay renderer
```

No application may acquire a generic DOM framework, and the shared browser
renderer may not acquire Book, Crèche, Patchbay, execution, authority, Plan, or
Play truth. A rendered element and a browser event are presentation identities,
not semantic or runtime identities.

## Responsibility classes

| Class | Owns | Must not own |
|---|---|---|
| Application/model | Meaning, application state, actions, projections, and content | DOM nodes, browser event listeners, device APIs, or generic rendering |
| Renderer-neutral presentation | Finite component descriptions, stable presentation keys, theme roles, and finite application-event values | HTML strings, selectors, browser handles, semantic authority, or runtime scheduling |
| Browser Host presentation mechanism | Description validation, DOM manifestation, event capture, focus, and bounded event delivery | Application meaning, action policy, target fabrication, or execution truth |
| Browser Base/device mechanism | Exact browser-provided effects such as WebUSB and media access behind admitted Host operations | Application presentation policy or authority inference |
| Target-owned adapter | Exact target descriptors, tooling, artifacts, loading, and flashing behavior | Generic application rendering or Host scheduling |
| Temporary compatibility shell | Existing page bootstrap, routing, DOM assembly, and duplicated controls needed during migration | A second permanent renderer or new public contract |

## Current topology and intended ownership

### Browser Host

| Current surface | Current responsibility | Destination | Phase |
|---|---|---|---|
| `targets/browser/host/src/main.rs` and `src/server/surface.rs` | Hard-coded `Host`, `Book`, and `Creche` personalities, page allowlists, and entrance selection | Replace application personalities with loading of finite application artifacts; keep Host launch and serving mechanics | 4-5 |
| `targets/browser/host/src/server.rs` | HTTP serving, runtime bytes, Book/Crèche documents and scripts, browser Bases, and target adapters in one router | Keep Host transport and browser mechanisms; move application selection to the shared artifact boundary; retain target adapters in their owning target/browser layer | 1-5 |
| `targets/browser/host/assets/browser-host-bootstrap.mjs` | Browser Host bootstrap | Host-owned mechanism; extend only through the bounded shared protocol | 1 |
| `device-base.mjs`, `usb-device-base.mjs`, `media-host.mjs`, `websocket-line.mjs` | Browser device, media, and Line effects | Remain Host/Base-owned and outside the presentation protocol | all |
| `targets/browser/host/src/server/book_assets.rs` | Serves the production Patchbay HTML renderer assets into Book | Preserve reuse of the real Patchbay renderer; replace the Book-specific asset bridge only after the shared Host renderer serves it | 2, 4-5 |

### Book

| Current surface | Classification and split | Destination | Phase |
|---|---|---|---|
| `products/tour/content/` | Book-authored lessons and examples | Remain Book application content | 2 |
| `targets/browser/runtime/src/book_runner/` | Book execution/application bridge | Remain application/runtime behavior; emit bounded application state rather than DOM instructions | 2 |
| `targets/browser/host/assets/book.mjs` | Mixed lesson navigation, application actions, markdown/content projection, bespoke DOM construction, and embedded Patchbay coordination | Keep Book state/actions/content projection; replace generic DOM work with shared presentation descriptions and events | 2 |
| `book.html` and `book.css` | Compatibility document and a page-specific control/theme system | Reduce to Host bootstrap and Book-specific layout roles; remove duplicated generic controls and copied theme values | 2, 5 |
| Book Patchbay assets and `flow*.js` served by `book_assets.rs` | The actual browser Patchbay faceplate renderer, not a lookalike | Continue using the same renderer selected for Patchbay HTML; converge its transport and lifecycle with the Host-owned renderer | 2, 4 |
| Book staging and browser specifications | Packaging and acceptance proof for the current compatibility shell | Update incrementally to prove the shared boundary, then remove compatibility-only staging | 2, 5 |

Book migration must preserve lesson routes, navigation, executable examples,
real Patchbay inspection, gear face/back flipping, and animated Cord activity.
Those are application acceptance behaviors; they are not permission to duplicate
the Patchbay renderer inside Book.

### Crèche

| Current surface | Classification and split | Destination | Phase |
|---|---|---|---|
| `targets/browser/host/assets/creche-lifecycle.mjs` and `creche-graduation.mjs` | Crèche lifecycle state, application actions, and their current DOM projection are mixed | Keep lifecycle/actions in Crèche; express views and events through the shared protocol | 3 |
| `creche.mjs`, `creche.html`, and `creche.css` | Bespoke shell, routing, DOM helpers, and controls | Replace generic shell/rendering with Host mechanism; retain only Crèche-specific presentation roles | 3, 5 |
| `creche-existing-computer.mjs` | Existing-computer application workflow mixed with browser interaction | Split application choices from Host-owned browser effects | 3 |
| `creche-target-catalog.mjs` | Target catalog projection and selection | Keep semantic catalog/selection in Crèche; do not move target truth into the renderer | 3 |
| `creche-physical.mjs` | Physical-host orchestration mixed with browser device APIs | Keep application sequence in Crèche and browser APIs behind Host/Base operations | 3 |
| `creche-release-bundle.mjs` and `creche-spore-bundle.mjs` | Bundle presentation and target-specific delivery coordination | Keep target facts and delivery adapters target-owned; render their state through the shared boundary | 3 |
| Crèche staging and browser specifications | Current package/proof shell, including work overlapping PR #2054 | Change only after #2054 lands or with an explicit handoff; then migrate proof without weakening artifact or physical distinctions | 3, 5 |

Crèche application loading may not turn reachability into membership, trust, or
authority. Presentation events request Crèche actions; they never grant access
to a device or select a Host implementation by themselves.

### Patchbay

| Current surface | Classification and split | Destination | Phase |
|---|---|---|---|
| `products/patchbay/model/src/` | Renderer-neutral Patchbay state, actions, projections, layout, and theme | Remain the authoritative application/presentation model used by browser and native renderers | 1, 4 |
| `products/patchbay/model/src/theme.rs` | Fixed, bounded, toolkit-independent theme roles explicitly excluded from semantic identities | Generalize or narrowly wrap as the shared presentation-theme nucleus; preserve identity exclusion | 1 |
| `products/patchbay/html/assets/flow.js`, `flow-scene.js`, `flow-layout.js`, and `flow-faceplate.js` | Actual React Flow browser manifestation of Patchbay projections | Become one Host-owned browser Patchbay renderer used by both Patchbay HTML and Book | 4 |
| `products/patchbay/html/assets/app.js` | Patchbay action orchestration mixed with generic DOM construction and updates | Keep Patchbay actions/projections in the app; move generic manifestation and event plumbing to the Host | 4 |
| `panel-furniture.js`, `index.html`, and `app.css` | Generic panels, controls, shell, and duplicated styling | Replace with shared components/theme roles; keep only Patchbay-specific layout | 1, 4-5 |
| `products/patchbay/html/src/server.rs` and `src/server/*` | Patchbay HTTP compatibility server, application endpoints, theme transport, and asset serving | Preserve application endpoints until Host application loading is complete; retire duplicate HTTP/shell delivery in Phase 5 | 4-5 |
| `products/patchbay/native/src/` | Native renderer and native interaction adapter | Retain as a first-class independent renderer; keep consuming renderer-neutral model/theme | all |

## Shared protocol boundary

Phase 1 must define one versioned, finite, renderer-neutral protocol before any
application migrates. Its initial proof is a tiny non-Patchbay view so the
boundary cannot be reverse-engineered around React Flow.

The protocol may carry:

- a finite tree or similarly bounded component description using an enumerated
  component vocabulary and stable presentation-local keys;
- text and attributes with admitted byte limits;
- theme roles, not raw application CSS or semantic identity inputs;
- finite action descriptors that map browser events to application-event values;
- an explicit description revision so stale events refuse deterministically.

The protocol must not carry raw HTML for ordinary rendering, DOM nodes,
selectors, JavaScript callbacks, device handles, URLs as authority, Forms,
Plans, Plays, or implicit application dispatch. Any exceptional opaque browser
surface, including the Patchbay canvas, must be an enumerated component with a
bounded renderer contract rather than an HTML escape hatch.

Before manifestation, validation must admit explicit maxima for description
bytes, component count, depth, text/attribute bytes, actions, and resources.
Event delivery must admit explicit maxima for encoded bytes and queued events.
The exact numeric limits belong to the Phase 1 implementation and tests; this
inventory deliberately does not invent them.

Machine-readable refusal must distinguish at least:

- unsupported protocol version, component, attribute, event, or theme role;
- malformed encoding, duplicate key, invalid parentage, depth excess, and
  description/count/byte excess;
- stale description revision, unknown action, event byte excess, and event
  queue pressure;
- unavailable renderer or opaque-surface implementation.

Refusal occurs before partial DOM mutation. Event pressure is reported, not
hidden behind an unbounded browser queue, retry, or coalescing policy.

## Migration sequence and stop lines

1. **Phase 0 — inventory.** Land this boundary map only. It changes no runtime,
   UI, route, claim, or application ownership.
2. **Phase 1 — nucleus.** Add the finite protocol, shared theme-role transport,
   Host DOM renderer, negative conformance tests, and one tiny non-Patchbay
   proof. Do not migrate Book, Crèche, or Patchbay yet.
3. **Phase 2 — Book.** Move one Book vertical slice at a time, retaining the real
   Patchbay renderer and all current lesson behavior. Do not touch Crèche target
   adapters.
4. **Phase 3 — Crèche.** Begin only after PR #2054's native ZIP/disk ownership is
   merged or explicitly handed off. Preserve target/Host boundaries and proof
   classes.
5. **Phase 4 — Patchbay HTML.** Make the standalone HTML application load through
   the same Host runtime and Patchbay browser renderer used by Book. Keep native
   Patchbay green.
6. **Phase 5 — removal.** Delete only compatibility routes, servers, shell code,
   duplicated controls, and copied theme rules proven unused after all three
   migrations. Update claim documents only with exact-main evidence.

Each phase is a separate reviewable PR based on current `main`, with its own
allowlist and exact-head checks. A later phase does not enter an earlier PR just
because a convenient seam appears.

## Acceptance matrix

| Contract | Phase 1 | Book | Crèche | Patchbay HTML | Native Patchbay |
|---|---:|---:|---:|---:|---:|
| Bounded description accepts and renders | required | required | required | required | n/a |
| Malformed/oversized description refuses before mutation | required | regression | regression | regression | n/a |
| Bounded event round trip and stale/pressure refusal | required | required | required | required | n/a |
| Shared theme roles and accessible text contrast | required | required | required | required | required model-token regression |
| No raw HTML ordinary-rendering path | required | required | required | required | n/a |
| Application semantics and authority unchanged | tiny proof | required | required | required | required |
| Existing application journey | n/a | all Book lessons | birth through graduation | existing Patchbay flows | existing native flows |
| Same browser Patchbay renderer | n/a | required | n/a | required | distinct renderer |

Browser acceptance remains one pinned Chromium project, one worker, zero
retries, and no forced interaction. Native renderer tests remain mandatory for
Patchbay model, theme, layout, and interaction changes. A browser pass does not
establish native, firmware, physical, or human proof.

## Deletion ledger

The observed compatibility surface is approximately 4,300 lines of browser
JavaScript and CSS across Book, Crèche, and Patchbay HTML. This is an inventory
baseline, not a deletion quota: application behavior and the real Patchbay
renderer must survive.

Likely deletion or shrinkage candidates after migration are:

- Book and Crèche generic DOM helpers, shell bootstrap, duplicated buttons,
  panels, status furniture, focus behavior, and copied theme rules;
- Patchbay HTML generic panel furniture and duplicate shell/control styling;
- browser Host `ProductSurface`/`ProductDocument` personalities and per-app
  asset routing;
- Patchbay's duplicate standalone browser delivery shell after Host loading is
  the accepted entrance;
- compatibility-only staging code and tests once equivalent exact-Host proof
  exists.

Application content, state machines, action policy, target adapters, Patchbay
model/projections, the React Flow faceplate renderer, and native Patchbay are not
deletion targets. Every removal PR must report before/after line counts for its
owned files and identify retained responsibilities; raw line reduction cannot
justify loss of behavior or a parallel implementation.

## Phase 0 completion

Phase 0 is complete when this map is reviewed against the live source tree and
linked from issue #2050. It establishes vocabulary, ownership, sequencing,
negative boundaries, overlap with PR #2054, and the proof matrix. It does **not**
complete any issue #2050 product acceptance criterion, authorize deletion, or
claim that Book, Crèche, and Patchbay already use one Host-owned runtime.
