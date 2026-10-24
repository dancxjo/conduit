# GUI surface inventory for #6349

Initial audit against `75eeb19bf`; owner chrome coverage refreshed at
`589a38bcb`. Source-specific evidence is recorded below and in the progress audit. This is a coverage checklist, not an acceptance receipt. A label is
always retained; icons are decoration and do not select actions or grant authority.

## Shared vocabulary and fallbacks

`mechanisms/implementations/bounded-lucide/gui-vocabulary.json` owns exact
role/intent-to-icon mappings. Generated browser geometry and native enum/index
mappings consume the pinned Lucide subset. Native preparation uses 28 finite
192×192 one-bit masks (129024 bytes); browser inline SVG uses the same silhouettes.
The owner Face component never guesses intent from captions or opaque action IDs.
Unknown role/intent remains complete text without an invented symbol. Missing Look
retains default controls; unsupported native geometry falls back to the measured
facts reader. Missing admitted resources remain resource failures, not semantic
success. All distributed applications must carry the Lucide notice.

| Meaning | Shared icon | Labels and fallback |
| --- | --- | --- |
| Body / Plot / Document | Box / Presentation / Type | Source name remains the heading. |
| Host / Gear / Port / Cord | Server / Settings / Plug / Cable | Exact identities/facts remain readable in Details. |
| Plan / Play / status / diagnostic | List checks / Play / Info / Circle alert | Text gives actual state or refusal; color alone is insufficient. |
| Birth / Wake / Lull | Sunrise / Sunrise / Moon | Exact offered versioned intent required. |
| Name / naming / suggestion / search | Type / Settings / Repeat / Search | Field labels and native editor mechanics remain authoritative. |
| Inspection / library / collection | Scan search / Library / Layers | No icon invents a popup, route or collection. |
| Complete / reopen | Check / Repeat | Item-specific accessible action name is retained. |
| Close / back / confirmation | X / Arrow left / Check | Existing controls keep visible labels or explicit accessible names. |

## Concrete surface coverage

| Surface and source | Current decoration | Remaining work / supported boundary |
| --- | --- | --- |
| Browser Workspace shell (`workspace.html`, `workspace.css`) | Body lifecycle, membership/Host shortcuts, panel close controls; Crèche name/search/Plot/Birth masks | Invitation receiver now uses shared Body/Inspect symbols and 44px controls; min-content reflow is constrained without clipping. Bottom Plot/flow/lifecycle controls now use pinned Plot/Cord/Inspect/status symbols, retain labels through live selection updates, and have 44px minima. Actual Birth/Wake, Unicode editor retention, three keyboard inspection/focus-return round trips and three local chrome Looks are proven at `72df3b659`; canonical resident Look selection and editor font/surface integration remain distinct work. |
| Browser owner Face (`handbook/owner-face-component.mjs`) | Shared role headings, offered action intents, collection/choice legends, completed/empty states, relationship/inspection/facts disclosures | Arbitrary domain roles/intents intentionally stay text-only. Real owner action receipts remain distinct from renderer fixtures. |
| Browser owner participation (`handbook/owner-participation.mjs`) | Embedded Face inherits the shared renderer; Host/Body/Face headings, join, refresh, wardrobe inspection/policy, reading playback/status/stop, leave and evidence disclosures use fixed shared icons. Controls have 44px minima, visible focus, wrapping and narrow identity stacking. | Copy-public-key remains labeled text without a misleading substitute icon. These local operation mappings remain separate from owner-offered Face intents. Separate density/palette controls retain the acknowledged checked layout; typography selection remains preset-bound. |
| Browser Patchbay shell (`patchbay-workbench/assets/index.html`) | Library, Inspect, Body, canvas navigation/zoom/fit controls | Cancel placement, linear-view toggle, exact facts disclosures and Plan/Play/route/Host/linear evidence headings now carry pinned symbols. The shell keeps its existing HTML size ceiling; inline geometry and CSS do not require image loading or a CDN. Drawer stacking, bottom docking and enlarged control reachability are now checked at 390px with 32px root text and forced colors. Narrow masthead/navigation now follows source order with measured non-overlapping bands. Product links and actual Fit activation pass at 390px / 32px forced-color text, with 320px captures as supporting evidence. Full zoom and dynamic graph/list controls still need broader coverage. |
| Browser Patchbay diagram and Gear palette | Existing accepted finite Gear icons; shell additions use pinned inline SVG | Verify dynamic node/port/status/selection coverage with current runtime, not the retained mixed-source preview. |
| Browser Handbook local content | Embedded Face uses the shared component | Audit local navigation/status/refusal controls before declaring ordinary-GUI coverage complete. |
| Native Crèche (`native_face_scene/document`, checked Crèche recipe) | Body/Plot/collection/status headings, name/naming/search/suggestion/Birth intents; cream/navy or dark surfaces | Latest search/collection changes captured in normal-product QEMU at Source `dd5349718`, with chained fresh local Shows and unchanged Face revision. Checked card insets are retained at `6034b2c17`; 640×480 pointer/keyboard proof passes. Further hierarchy and native large-text/forced-color enactment remain. |
| Native ordinary Face reader and Details | Shared role/intent icons, finite measured text, explicit facts and refusal text | Verify representative non-Crèche Body with same Look; do not count Crèche reuse alone as Todo/Patchbay acceptance. |
| Native graph (`native_face_scene/diagram.rs`) | Exact Gear/Port/Cord geometry and labels; graph navigation preserves identities | Gear cards now use the shared Gear symbol within measured title boxes. Decoration consumes only residual commands after all prior cards/Port rows/notices/Cords, with a coherent text-only fallback for dense/small pages. Port rows now use measured spare width and residual commands for Plug symbols, with coherent text-only fallback. Cards use available viewport width, and shared hit/Cord geometry follows. Normal QEMU at `a161ac474` shows 4 Gear/7 Port symbols and all 3 Cords; native enlarged/forced-color/fallback and broader cross-component Look acceptance remain. The subsequent compositor custody collector refuses an ambiguous embedded ELF, so full graphical-profile acceptance remains open. |
| Native Commons/front door | Source-owned headings and typed entry actions | Inspect actual normal-product capture separately; Crèche icon evidence does not establish Commons coverage. |
| Native header/footer (`native_face_scene/chrome.rs`, `frame.rs`) | Real page marker/proportional page line and keyboard help | Keep continuation truthful. Any icon must fit measured chrome and retain compact/large-text fallback. |

## Required visual and interaction checks

- Browser: actual runtime at wide/narrow widths, long Unicode values, enlarged text,
  keyboard and pointer, unavailable controls, forced colors, reduced motion and
  accessible names. Axe is supporting evidence, not full accessibility acceptance.
- Native: normal product QEMU, wide/narrow and selected large text; keyboard/pointer
  through acknowledged Shows, text and icon resource bounds, exact correspondence
  after changing Look. CPU raster/layout checks are not enacted QEMU interactions.
- Cross-component: same selected Look on Todo action rows and Patchbay nodes with
  their different checked structures. Local Workspace chrome preferences and sample
  screenshots do not prove canonical owner Look selection or a shared Body.
- Preserve source, selected Face/revision, checked recipe, Show chain and action
  targets beside screenshots. Record deliberate differences from #6174's reference.
- Publication/integration remains the original owner contract. This draft uses
  local proof and skips CI while visual work continues; no parent closure claim.
