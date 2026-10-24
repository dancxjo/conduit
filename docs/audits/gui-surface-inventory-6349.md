# GUI surface inventory for #6349

Initial audit against `75eeb19bf`; owner chrome coverage refreshed at
`589a38bcb`. Source-specific evidence is recorded below and in the progress audit. This is a coverage checklist, not an acceptance receipt. A label is
always retained; icons are decoration and do not select actions or grant authority.

## Shared vocabulary and fallbacks

`mechanisms/implementations/bounded-lucide/gui-vocabulary.json` owns exact
role/intent-to-icon mappings. Generated browser geometry and native enum/index
mappings consume the pinned Lucide subset. Native preparation uses 30 finite
192×192 one-bit masks (138240 bytes); browser inline SVG uses the same silhouettes.
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
| Browser Workspace shell (`workspace.html`, `workspace.css`) | Body lifecycle, membership/Host shortcuts, panel close controls; Crèche name/search/Plot/Birth masks | Invitation receiver now uses shared Body/Inspect symbols and 44px controls; min-content reflow is constrained without clipping. Bottom Plot/flow/lifecycle controls now use pinned Plot/Cord/Inspect/status symbols, retain labels through live selection updates, and have 44px minima. Actual Birth/Wake, Unicode editor retention, three keyboard inspection/focus-return round trips and three local chrome Looks are proven at `72df3b659`; The native textarea now uses shared Host/Look text/surface tokens instead of inheriting the giant signal font, with actual retained-editor/selection/identity and 390px/32px forced-color proof; canonical resident Look selection remains distinct work. |
| Browser owner Face (`handbook/owner-face-component.mjs`) | Shared role headings, offered action intents, collection/choice legends, completed/empty states, relationship/inspection/facts disclosures | Arbitrary domain roles/intents intentionally stay text-only. Real owner action receipts remain distinct from renderer fixtures. |
| Browser owner participation (`handbook/owner-participation.mjs`) | Embedded Face inherits the shared renderer; Host/Body/Face headings, join, refresh, wardrobe inspection/policy, reading playback/status/stop, leave and evidence disclosures use fixed shared icons. Controls have 44px minima, visible focus, wrapping and narrow identity stacking. | Copy-public-key remains labeled text without a misleading substitute icon. These local operation mappings remain separate from owner-offered Face intents. Separate density/palette controls retain the acknowledged checked layout; typography selection remains preset-bound. |
| Browser Patchbay shell (`patchbay-workbench/assets/index.html`) | Library, Inspect, Body, canvas navigation/zoom/fit controls | Cancel placement, linear-view toggle, exact facts disclosures and Plan/Play/route/Host/linear evidence headings now carry pinned symbols. The shell keeps its existing HTML size ceiling; inline geometry and CSS do not require image loading or a CDN. Drawer stacking, bottom docking and enlarged control reachability are now checked at 390px with 32px root text and forced colors. Narrow masthead/navigation now follows source order with measured non-overlapping bands. Product links and actual Fit activation pass at 390px / 32px forced-color text, with 320px captures as supporting evidence. Status, observation, run and workspace tools now occupy a measured footer outside the graph; the exact omitted-Cord notice is retained there. Twelve current HTML/furniture checks cover semantic identity, delivery loss, focus return and enlarged forced-color reachability. Broader authoring checks still fail and remain unaccepted; full zoom and dynamic graph/list controls need broader coverage. |
| Browser Patchbay diagram and Gear palette | Existing accepted finite Gear icons; shell additions use pinned inline SVG | Nodes now lower canonical icon wire tokens through the shared pinned SVG path; Port and status/fault symbols use that same vocabulary. Source-bound documentary workbench checks retain keyboard Gear selection, pointer Port selection, unchanged Face under 390px/32px forced colors, and unknown-token text fallback. Direct Port selection now uses native buttons with complete names, server-derived pressed state and shared pointer/Enter/Space binding. Tab entry and exactly one request per keyboard activation pass at `ed9c3db6a`; 44 CSS-pixel row minima scale with the canvas. These captures do not establish a newly born live Body or native acceptance. |
| Browser Handbook local content | Fixed local Wake/Lull, Patchbay, return, source download/inspection, Try, Body inspection, release/reset/retry and heading icons reuse the shared pinned renderer. Labels and native controls remain. | [Actual module recovery captures](evidence/gui-look-6349-handbook-chrome/README.md) pass wide and 390px/32px forced-color editing/download/disclosure, icon/target and no-overflow checks with an explicit opening refusal. This does not prove a live Handbook Body or actual storage failure. Dynamic status/refusal text remains explicit; broader successful-lifecycle coverage remains. |
| Native Crèche (`native_face_scene/document`, checked Crèche recipe) | Body/Plot/collection/status headings, name/naming/search/suggestion/Birth intents; cream/navy or dark surfaces | Latest search/collection changes captured in normal-product QEMU at Source `dd5349718`, with chained fresh local Shows and unchanged Face revision. Checked card insets are retained at `6034b2c17`; 640×480 pointer/keyboard proof passes. Further hierarchy and native large-text/forced-color enactment remain. |
| Native ordinary Face reader and Details | Source 18f653cf4 adds existing explicit role/intent icons to Details subject/action clauses; contracts/properties and unknown mappings retain text. Exact wording/read-only checks pass. | Actual 640×480 / 200% / forced-color Details completes all 56/71 pages and sixteen typed controls in separate bounded Look runs. Representative non-Crèche Body with the same Look, Details pointer and unavailable/resource fallback still need their own acceptance. |
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

## Todo add/remove follow-up

Exact `todo/add@1` and `todo/remove@1` now use the shared pinned plus and trash
icons in browser and native mappings. Existing complete/reopen icons remain.
[Local renderer preview and proof boundaries](evidence/gui-look-6349-todo-icons/README.md)
cover accessible names, exact invocation payloads and unknown-intent fallback.
The native asset generator reproduces all 30 masks while preserving the old 28
byte-for-byte; current-source native runtime/QEMU proof remains open.

## Default Look and control typography follow-up

A Look without visual policy now selects canonical Phosphor, retaining the
requested multimodal resource and a `MissingVisualFacet` reason. Native checked
Source/CPU preparation preserves the Face and complete action controls.
Canonical browser action/field text now uses the Look body token (16px for
Phosphor), rather than a hard-coded 13px override; supporting text consumes the
small token. Argumentless actions retain one button label without a repeated
heading. [Before/after captures and exact proof boundaries](evidence/gui-look-6349-default-fallback/README.md)
include 390px forced colors and actual 32px control text.

An independent browser Source probe still fails before admission: the browser
catalog and configuration recovery accept registered Todo Source/roots. A live
nonvisual request, general checked-source/component reuse, and current native
QEMU remain unproved. Renderer fixtures must not be relabeled as those receipts.

## Registered checked Patchbay document

Browser preparation/recovery now recognizes a separate reviewed Patchbay
component Source and its Gear/Port/Cord recipe. All four Look resources are
byte-identical to Todo's; default absence also reaches the configured consumer.
[Source/custody checks and local captures](evidence/gui-look-6349-registered-patchbay/README.md)
cover a synthetic node/port Face through actual CPU Mask preparation, simulated
Show acknowledgement and the real renderer, including 390px forced colors and
32px controls. This is region grouping, not full graph-node canvas acceptance.
Owner controls remain Todo-specific. The complete 66-subject domain fixture
still refuses at the carrier bound; it is not trimmed for positive proof.
Arbitrary external Source, live owner-selected Patchbay and native QEMU remain
outside this evidence.

## Fresh native Source 92598cefd

[Actual current Crèche and diagram captures](evidence/gui-look-6349-current-native/README.md)
now bind 22 QMP PNG hashes to a fresh normal product Root/ISO. Four local Look
swaps preserve the complete Face, component, recipe and layout. All current
presets are typographically Editorial; legacy engineering filenames describe
Phosphor palette captures. [The reference comparison](creche-reference-comparison-6174.md)
now uses this actual Source and records remaining visual differences.

The image builds after narrowly updating standalone guest locks and fitting the
Application executable inside its unchanged image window. The journey then
fails at a later Patchbay prepare-target operation with Application WorkExhausted.
No full journey or compositor-lifecycle receipt follows; current narrow/forced/
200% native proof and actual Engineering selection remain open. This supersedes
neither the older Source receipts nor the original parent acceptance scope.

## Native typography selection follow-up

Crèche now has an independent local Engineering typography choice (F8), beside
F6 density and F7 palette. Each combination selects another complete canonical
Look through a reviewed Source root; it does not patch a prepared expression.
The same admitted Host fonts measure and paint the replacement before the old
Show retires. Other product states retain their existing F8 action binding.

The focused actual Source/offer test covers both Crèche structures and all four
density/palette combinations: component, recipe, layout, renderer and paint
remain identical across typography choices, while the Look and plan differ.
Actual native switching and retained-draft submission pass the focused CPU test
([logs](evidence/gui-look-6349-handbook-chrome/README.md)); [actual QEMU evidence](evidence/gui-look-6349-native-engineering/README.md) now
passes the ten-transition Look matrix at Source e34b12788. The full journey
still fails during Plan with Application WorkExhausted; no later diagram/editor
receipts follow for this Source. The source-specific older evidence above remains
unchanged. The failed speed-optimized decoder experiment is retained separately
in [its negative record](evidence/gui-look-6349-native-decoder-experiment/README.md)
and was reverted; it did not fix Application WorkExhausted.

## Fresh native Engineering Source e34b12788

[The actual 1280×800 captures and scoped receipt](evidence/gui-look-6349-native-engineering/README.md)
prove independent Editorial/Engineering typography, paper/dark palette and
compact/spacious density on the complete unchanged Crèche Face and checked
component/recipe/layout. F8 selects the already pinned Sans title; F6/F7 retain
that typography choice while changing the other axes. All ten transitions use
fresh chained locally acknowledged Shows, no Body action and no fallback.
The browser's cool Engineering preset is separate paint; these native captures
show Engineering typography over the existing paper and Phosphor palettes.

The full native journey again fails Application PrepareTarget during Plan,
before editor/Patchbay/Stop. Narrow/200%/forced-color local Crèche, complete
continuation reachability, the closer navy reference, and original parent
acceptance remain open. The earlier source-specific records above are retained.

## Local Crèche user constraints follow-up

The local Crèche now accepts F9 for a 100%/200% user minimum and F10 for forced
colors, independently of F6 density, F7 palette and F8 typography. User constraints
are applied through the existing Host admission path. Clearing the 200% minimum
reveals the requested Look scale (125% for spacious), rather than mutating the
Look to 100%. Each replacement retains Face/actions and uses a fresh local Show;
F9/F10 retain the same selected Mask Plan and admitted resources.

The focused 640×480 CPU interaction case passes eighteen replacements, including
200%, forced colors, palette/type/density changes, override clearing and current-
Show draft submission, without layout fallback. The supported native controls
proof adds an accessibility scenario for long Unicode input and two actual
Looks, full primary control cycles/pages, pointer focus and incomplete-input
refusal. Its QEMU execution/captures are pending; it does not attempt Body startup
or claim complete Details, all unavailable controls, physical or human proof.

The initial actual accessibility run reached both 200%/forced-color Looks, then
[refused a split interactive field](evidence/gui-look-6349-local-a11y-field-split/README.md).
The reusable mechanical paginator now keeps a fitting action/argument pair
atomic when the full help/field subject is oversized; help retains ordinary
continuation. No text/control reordering, clipping or limit increase is used.
All 45 layout CPU cases and the eighteen preference replacements pass; fresh
QEMU validation of this pagination correction is pending.

## Native local accessibility Source 7fb598899

[Both actual normal-product QEMU runs pass](evidence/gui-look-6349-local-a11y/README.md)
at 640×480 and 1280×800. The same Root/ISO realizes both viewports. Each run
commits the exact long Unicode name, applies 200% text/forced colors in two
Looks, checks twelve primary controls in complete forward/reverse cycles with
the same exact cyclic order, visits every primary page, validates real pointer
focus and incomplete-input refusal, then clears user overrides without losing
Look intent. All seven replacements preserve complete Face, checked structure,
current local Show correspondence and no fallback. Each run retains 13 actual
PNGs inside the 27-capture profile; narrow/wide frame command high-water values
are 20/48, respectively, under the unchanged 48-command cap. This supersedes the
local primary narrow/large-text/forced-color pending status above for this Source.

Complete Details, every unavailable control, missing native assets, the closer
navy reference, live Look reuse on the full Patchbay node structure, and the
full Body/Patchbay/Stop journey remain separate open work. The later Application
work refusal has not been claimed fixed. Original parent acceptance remains.

## Discoverable native Look options

Source cb1efa2f5 adds measured local Crèche shortcut guidance for density, palette,
typography, user text minimum and forced colors. The actual 1280×800 accessibility
scenario passes with the full footer visible; geometry/priority CPU checks pass
without new layout space or commands. [Exact source, captures and limits](evidence/gui-look-6349-native-look-guidance/README.md).

## Large-text native icon rhythm

Source dfeb14b59 scales existing semantic row icons and their reserved gap with
admitted text size across flat/Details, panel and mechanical layouts. 147 native
scene cases and the eighteen-replacement Look/draft case pass. Actual wide and
narrow QEMU primary accessibility scenarios pass at 200%/forced colors.
[Before state, current captures and precise boundaries](evidence/gui-look-6349-native-icon-scale/README.md).

## Wide native Details correspondence

Source 8b9f820f9 passes actual 1280×800 Details at 200%/forced colors in both
typography/density/palette combinations: sixteen available typed controls in
complete forward/reverse cycles, all nineteen/twenty-three pages, explicit F2
entry/return and unchanged complete Face/Show during navigation.
[Exact receipts, initial inventory refusal and remaining narrow/fallback scope](evidence/gui-look-6349-native-details/README.md).

## Narrow Details icon and continuation follow-up

Source 18f653cf4 closes the scoped narrow Details keyboard/page gap using explicit
one-Look bounded requests: all sixteen controls, fifty-six Editorial and
seventy-one Engineering pages, with existing role/intent icons and no wording
loss. Raw record/serial/command/capture bounds are unchanged.
[Actual captures, source identity, initial profile refusal and precise remaining scope](evidence/gui-look-6349-native-details/README.md#narrow-details-and-shared-icons--source-18f653cf4).
