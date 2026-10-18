# Measured native Face layout (#5606)

Development proof record on accepted base `a2ee4ebea348ec9bcb6725a79e0a23d4a2ca300d`,
including #5963's editor and #5968's source-equivalent control validation.
Final source-pinned native proof, workspace results and candidate admission
must be recorded before closure.

## Ownership and disclosure

The immutable Face supplies every name, status sentence, relationship, action,
target, argument contract and disclosure. Layout stores only measured rows,
pages, focus and bounded Mask-local input. It creates no Body, action, lifecycle
fact or universal widget meaning.

| Encounter/item | Face provenance and priority | Native region/role | Control and overflow |
|---|---|---|---|
| Zero-Body Crèche | Canonical birth/draft Face; original subject and clause order | Source title, labels and available actions in primary rows | Exact action/argument indices; continuation and F2 Details |
| Born Home | Historical Home Face in `proof/native-shell/fixtures.json`, reviewed at `7989c14248d479715b5bd6e5379da11eb617fa87` | Same generic primary/detail policy | Existing missing launcher-action coverage remains #5604; no invented Home actions |
| Lulled/awake Body | Canonical owner-derived Body Face; name followed by its supplied text | Title then Status; source wording is unchanged | No lifecycle inference; all original clauses remain in Details |
| Generic/unknown domain | Open roles, supplied text and explicit disclosure | Existing title/heading/status/warning/label roles | Available actions only; typed arguments retain their contracts |
| Exact secondary facts | Complete existing utterance plan, identities, relationships and unavailable-action reasons | Detail rows | UTF-8-safe continuation; original wording reconstructs exactly |
| Page marker and instructions | Renderer-owned reading state/instructions | Measured header/footer | Compact wording changes no Source subject, intent or authority |

Body text previously disappeared from the primary view even when the owner
supplied its current status. It now follows the Body name using the existing
Status typography role. Technical context/properties still remain in Details
according to the existing disclosure policy.

The installed native Crèche uses `NativeFaceScene`. Born Body and Home still
have older shell views; shared-layout fixture coverage does not claim that
#5604's semantic convergence has happened. Their real QEMU captures and the
canonical Face fixture checks are separate evidence classes.

## Geometry and finite admission

`chrome.rs` owns pure measured header/footer geometry. `layout.rs` consumes its
content interval; `frame.rs` assembles the admitted rows and exact hit mappings.
The root module retains its public paths and preparation/navigation façade.
Chrome does not depend on orchestration or on Plot evaluation. The legacy
header/footer constants are re-exported for independent Patchbay geometry.

Viewport limits remain 320×240 through 16,384×16,384. Each dimension is validated
before preparation. Header/footer heights use the same proportional Unicode
`TextLayout` cursor and admitted scale as rasterization. The largest possible
page marker is measured before pagination: at most 32,768 rows imply at most
32,768 pages. This avoids circular page-count/height admission.

The following content intervals are shared by Crèche, Home, Body and generic
Face fixtures. Coordinates are half-open vertical pixel intervals; footer
height includes its separator and spacing. Source rows are measured separately
inside the interval, retaining their order and controls.

| Viewport | Scale | Before header/content/footer | After header/content/footer |
|---|---:|---|---|
| 1280×800 | 100% | `[0,56)` / `[56,744)` / `[744,800)` | `[0,56)` / `[56,744)` / `[744,800)` |
| 1280×800 | 200% | `[0,56)` / `[56,744)` / `[744,800)` | `[0,64)` / `[64,744)` / `[744,800)` |
| 640×480 | 100% | `[0,56)` / `[56,424)` / `[424,480)` | `[0,56)` / `[56,424)` / `[424,480)` |
| 640×480 | 200% | `[0,56)` / `[56,424)` / `[424,480)` | `[0,56)` / `[56,424)` / `[424,480)` |
| 320×240 | 100% | `[0,56)` / `[56,184)` / `[184,240)` | `[0,56)` / `[56,184)` / `[184,240)` |
| 320×240 | 200% | `[0,56)` / `[56,184)` / `[184,240)` | `[0,84)` / `[84,184)` / `[184,240)` |

The old allocated heading/marker/footer text heights were 32/24/40 pixels at
every scale. At 200%, the measured heading and marker exceed those bounds;
narrow footer instructions also exceed 40 pixels. New text boxes have measured
header capacity 48/40/68 pixels at 1280/640/320 widths respectively at 200%.
The marker has its own non-overlapping column. Compact renderer-owned wording
fits the measured footer; full original Source wording is never shortened.

Below 640 pixels, a permitted two-column recipe admits one reading column.
Wider geometry may use up to two columns. A recipe's column maximum does not
make narrow two-column geometry mandatory or change semantic reading order.
Spacing and text scale stay bounded; there is no character-count width guess.

Each page reserves six common drawing commands: background, heading, marker,
two separators and footer text. Admission charges each row's text, region
marker, control outline and heading accent before placing it. Page pressure
creates another finite page before exceeding 48 commands. Twenty rows is still
an upper bound, not a requirement to fit twenty decorated controls on one page.

The original synthetic dense reproducer has eight title regions and typed
controls. Before the change, preparation succeeded but page zero returned
`SceneBound` at 1280×800 at both 100% and 200%. Command-aware admission exposes
continuation instead. The new mixed-availability test also covers multiple
arguments per action and exact source control mapping.

## Text, controls and refusal

Text commands remain at most 192 UTF-8 bytes. Segmentation checks scalar
boundaries and actual measured height, including long uninterrupted words.
Details retains exact original clauses and reconstructs long Unicode names,
combining sequences, accents, wide punctuation and unsupported scalars. Glyph
replacement changes rasterization only; it never replaces the Source string.
This does not claim complex-script shaping.

Every hit rectangle is a painted control rectangle resolving to the same
available Face action and optional argument. Reflow preserves Face identity,
revision, subjects, actions and contracts. A new acknowledged Mask Show remains
necessary for producer custody; the CPU producer/compositor proof retires the
old Show before admitting the next extent and observes three distinct Shows
for the same retained Face. Layout does not dispatch an owner action.

An unmeasurable local notice returns `InputBound` before replacing the prior
notice/frame. An oversized input preview also refuses instead of returning a
complete clipped frame. Existing clearing/cancellation APIs remain available;
complete focus, input-overflow usability and refusal presentation belong to
#5607. Surface/provider-loss acceptance belongs to #5608.

Patchbay Gear/Port/Cord positioning and endpoint/order logic are unchanged.
At narrow widths its renderer-owned notice abbreviates F2/F3 hints while
retaining exact shown/total/omitted counts. Independent diagram geometry keeps
its own fixed chrome envelope: expanded chrome that cannot fit returns
`InvalidExtent`; the same exact Face remains readable through F2 Details.
There is no claim that the independent diagram now supports 200% geometry.

## Bounds and proof classes

| Bound | Value/policy |
|---|---|
| Graphics commands per frame | 48, including chrome and decorations |
| Text payload per command | 192 UTF-8 bytes |
| Rows per document | 32,768 |
| Document text | 2 MiB per existing prepared document |
| Rows per page | At most 20, further reduced by geometry/command pressure |
| Columns | At most two; one below 640 pixels |
| Text scale | Existing admitted 100–200% |
| Retained compositor surfaces | Existing eight-surface ceiling; no additional surface is introduced |
| Extents outside admission | Explicit `InvalidExtent` |
| Unfit dynamic footer text | Explicit `InputBound`, no complete clipped frame |

The metric tests visit every primary/detail page of canonical Crèche,
lulled/awake Body and unknown-domain specimens plus the retained historical
Home Face at all three extents, both scale endpoints and both densities.
Read-only pages, exact Unicode reconstruction, mixed available/unavailable typed
controls, painted hit containment and stale interaction regressions are covered.
CPU raster tests use the identical scale, bounded pixel target and guard pixels.

The accepted active-editor facet consumes the same measured content interval.
Its one-line caption uses actual Label metrics, an explicit ellipsis when the
name cannot fit, and the existing end-of-document suffix. The complete original
name and document remain in Details; caret and tail-window meaning and editing
state remain the producer's. Compact editor instructions are measured along
with the other chrome variants. The editor geometry tests include long Unicode
names at both scales and all three extents.

The producer/compositor receipt proof uses an immutable owner Face and a
fixture surface provider. It executes the production forwarding/Mask kernel,
records actual publication Source/checked/expanded identities, Plan/Play,
Host/Boot, acknowledged Show and compositor/scene data, and retires custody
between extents. It is CPU production-path fixture evidence, not a live owner
attestation, domain action or QEMU capture. A forwarding receipt does not
authenticate the supplied owner facts.

The real before QEMU journey completed all 14 retained checkpoints at
1280×800. A separate normal q35/std-VGA probe verified actual 640×480 and
completed Details, next-page and return-to-primary inputs, with screenshots
fenced by guest redraw acknowledgements. A 320×240 preferred-mode probe actually
selected 1024×768; that receipt is an extent-mismatch diagnostic, not 320×240
scanout acceptance. The minimum 320×240 lane has metric/raster coverage and is
not mislabeled as a guest capture.

Final after-run source identities, output hashes, green workspace checks and
required candidate gates remain to be recorded. Native reproduction uses the
supported `cargo xtask make conduitos journey-proof` entrance. Internal proof
commands use strict workspace Clippy, full workspace tests with the native
compositor feature, and the focused native layout/producer tests. No physical
display, complete F2 convergence, published journey, stable release or parent
#5357 completion is inferred from these development artifacts.
