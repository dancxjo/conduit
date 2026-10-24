# GUI surface inventory for #6349

Audited against `75eeb19bf` (production code), with evidence checkpoint
`dd5349718`. This is a coverage checklist, not an acceptance receipt. A label is
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
| Browser Workspace shell (`workspace.html`, `workspace.css`) | Body lifecycle, membership/Host shortcuts, panel close controls; Crèche name/search/Plot/Birth masks | Invitation receiver now uses shared Body/Inspect symbols and 44px controls; min-content reflow is constrained without clipping. Bottom Plot/flow/lifecycle inspection buttons still mix text, Unicode arrows and a status dot. Audit their consistent icon treatment. |
| Browser owner Face (`handbook/owner-face-component.mjs`) | Shared role headings, offered action intents, collection/choice legends, completed/empty states, relationship/inspection/facts disclosures | Arbitrary domain roles/intents intentionally stay text-only. Real owner action receipts remain distinct from renderer fixtures. |
| Browser owner participation (`handbook/owner-participation.mjs`) | Embedded Face inherits the shared renderer | Join/copy-key/refresh/wardrobe/speech/leave chrome remains text-only. Add truthful local-operation icons without pretending those are owner-offered Face actions. |
| Browser Patchbay shell (`patchbay-workbench/assets/index.html`) | Library, Inspect, Body, canvas navigation/zoom/fit controls | Cancel placement, linear-view toggle and facts/route/Host sections remain text-only; dynamic graph/list controls need an inventory of their generating sources. |
| Browser Patchbay diagram and Gear palette | Existing accepted finite Gear icons; shell additions use pinned inline SVG | Verify dynamic node/port/status/selection coverage with current runtime, not the retained mixed-source preview. |
| Browser Handbook local content | Embedded Face uses the shared component | Audit local navigation/status/refusal controls before declaring ordinary-GUI coverage complete. |
| Native Crèche (`native_face_scene/document`, checked Crèche recipe) | Body/Plot/collection/status headings, name/naming/search/suggestion/Birth intents; cream/navy or dark surfaces | Latest search/collection changes captured in normal-product QEMU at Source `dd5349718`, with chained fresh local Shows and unchanged Face revision. Further inner card insets, aligned hierarchy and large-text/pointer enactment remain. |
| Native ordinary Face reader and Details | Shared role/intent icons, finite measured text, explicit facts and refusal text | Verify representative non-Crèche Body with same Look; do not count Crèche reuse alone as Todo/Patchbay acceptance. |
| Native graph (`native_face_scene/diagram.rs`) | Exact Gear/Port/Cord geometry and labels; graph navigation preserves identities | Diagram furniture is still text/paths, without the shared Gear/Port symbols. Add decoration within its existing command budget and measured text boxes. |
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
