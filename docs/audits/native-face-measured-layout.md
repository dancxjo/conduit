# Measured native Face layout (#5606)

Development proof for code `fe48e1f4901afe03992299d458788446c5ff1f58`, based on
accepted `a2ee4ebea348ec9bcb6725a79e0a23d4a2ca300d`, including #5963's editor and
#5968's source-equivalent control validation. The retained native CI product is
from merge preview `004075ae2e3b1b8146a9f0ae16c5990481988f0a`; its complete tree
is identical to that code commit (`e3e7c3c693517f282792faef71d3085bc2dba63c`).
This is development/emulator evidence, not stable release or physical acceptance.

## Ownership and disclosure

The immutable Face supplies every name, status sentence, relationship, action,
target, argument contract and disclosure. Layout stores only measured rows,
pages, focus and bounded Mask-local input. It creates no Body, action, lifecycle
fact or universal widget meaning.

| Encounter/item | Face provenance and priority | Native region/role | Control and overflow |
|---|---|---|---|
| Zero-Body Crèche | Canonical birth/draft Face; original subject and clause order | Source title, labels and available actions in primary rows | Exact action/argument indices; continuation and F2 Details |
| Born Home | Historical Home Face in `proof/native-shell/fixtures.json`, reviewed at `7989c14248d479715b5bd6e5379da11eb617fa87` | Same generic primary/detail policy | Historical launcher-action gap belongs to the recorded baseline; accepted #5972 supplies later wiring, without invented Home actions |
| Lulled/awake Body | Canonical owner-derived Body Face; name followed by its supplied text | Title then Status; source wording is unchanged | No lifecycle inference; all original clauses remain in Details |
| Generic/unknown domain | Open roles, supplied text and explicit disclosure | Existing title/heading/status/warning/label roles | Available actions only; typed arguments retain their contracts |
| Exact secondary facts | Complete existing utterance plan, identities, relationships and unavailable-action reasons | Detail rows | UTF-8-safe continuation; original wording reconstructs exactly |
| Page marker and instructions | Renderer-owned reading state/instructions | Measured header/footer | Compact wording changes no Source subject, intent or authority |

Body text previously disappeared from the primary view even when the owner
supplied its current status. It now follows the Body name using the existing
Status typography role. Technical context/properties still remain in Details
according to the existing disclosure policy.

At the recorded `fe48e1f…` code proof, native Crèche uses `NativeFaceScene`
while Body and Home retain older shell views. The accepted combination
`5abe26375c6e02cb769e412987e56e1baf386904` also consumes #5972, which changes
Home and lifecycle Source/Show wiring. Its new product proof is recorded
separately below. Historical fixture coverage is not relabeled as that later
installed route, and the upstream F2 audit owns its remaining unsupported cases.

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

## Retained current-source results

The fresh accepted-base CPU baseline visits 1,203 primary/detail pages and
finds 2,713 text commands taller than their allocated bounds, with no preparation
refusal. The measured policy visits 1,241 pages with zero clipped text and zero
frame refusals. Those five canonical/historical specimens use at most 35 commands;
the separate original dense reproducer admits all 56 frames, reaches exactly 48
commands, and has no frame refusal. The focused current-source suite passes all
50 tests; the production Mask/compositor custody trace passes separately.
Workspace all-target Clippy with the native compositor feature and warnings
denied passes. The full workspace unit, integration and documentation audit at that recorded
code revision exits zero: 9,468 passed, zero failed and 41 ignored. Ignored
platform/physical cases are not counted as accepted proof. All 1,116 ConduitOS
library tests pass with no ignores.

The actual 1280×800 supported QEMU journey comes from Candidate
[39112423375](https://github.com/dancxjo/conduit/actions/runs/39112423375), x86 job
117631438379, artifact 12131428296. The repository's `verifyBundle` checks all 26
product files against their exact inventory, sizes, SHA256 and preview identity.
The journey manifest is complete with 14 captured checkpoints and no failure,
including the accepted protected keyboard and retained Unicode editor facets.
The kernel SHA256 is `0104f1ce81948cd87c791a025246bddb286ec9d1b19935a7f1e46d4d90f4070f`;
the ISO SHA256 is `1f27e48b9b0a38d45f44f52fb716b32c723200fa1f88154085c60f917ac75148`.
In this original `004075a…` product, guest MASK_SIGN records keep absent
Show/Face facts as null; journey projection
Face IDs are not asserted to equal producer-acknowledged widget Shows. CPU
producer/compositor receipts prove that custody boundary separately with their
fixture provider. The source tree equality was checked before consuming that CI product; its
recorded preview identity is retained rather than relabeled as the branch head.

A separate local normal q35/std-VGA probe boots that same ISO, requests and
observes actual 640×480, and completes Details, next-page and return-to-primary
input stages. Each capture follows a guest redraw acknowledgement. Its receipt
records `captured`, `reading_completed: true`, no failure and process stop zero.
The inspected 1280/640 captures show text, controls and footer within their
regions. Full measured all-page assertions remain the geometric proof; a small
set of screenshots is not substituted for those assertions.

The historical before QEMU product remains pinned to `0c5a2a8…`; it is not
relabeled as the later `a2ee4eb…` baseline. Fresh before/after CPU measurements
use that later accepted base. Home/Body captures retain their installed-route provenance at that earlier
source. They do not establish the later wiring accepted in #5972; the current
product is verified separately. The 320×240 guest mode mismatch remains
a diagnostic only.

The source-pinned summary and raw inventory are retained in
[evidence/native-face-measured-layout-development-20261018.json](evidence/native-face-measured-layout-development-20261018.json)
and the [raw archive](evidence/native-face-measured-layout-development-20261018.raw.tar.gz). Source hashes, exact command text/bounds,
Face/control mapping tests, producer Source/checked/expanded/Plan/Play/Show and
Host/Boot receipts, guest serial/QMP/screenshots and original diagnostics remain
separately classified. Earlier full-workspace execution failed two screen-free
Birth tests; both pass in the final refreshed-source suite. Original failures,
the later cancelled superseded audit and the redundant cancelled local native
build are retained and are not counted as successful complete runs.

The original native CI product remains bound to its recorded source and
receipt. Current reproduction uses the normal Host product entrance below.
Internal verification uses workspace Clippy/full workspace tests with the native
compositor feature and the focused layout/producer tests. Implementation
[PR #5971](https://github.com/dancxjo/conduit/pull/5971) and its audit-completion
follow-up retain the corresponding Candidate and accepted-source CI records. No physical display,
complete F2 convergence, published journey, stable release or parent #5357
completion is inferred from these development artifacts.

### Accepted combination and current product

Implementation PR #5971 is accepted as `5abe26375c6e02cb769e412987e56e1baf386904`,
after numerical PR #5967 and lifecycle PR #5972. A complete diff verifies that
all layout, native Mask, typography/display and Face files are identical to the
recorded `fe48e1f…` code proof. The whole-workspace result retains its actual
source identity; it is not relabeled as the accepted combination. Fresh focused
validation on that accepted source is recorded separately.

Fresh accepted-source validation passes all 50 focused layout tests and the
production Mask/compositor custody test. Workspace all-target Clippy with the
native compositor feature and warnings denied also exits zero (7m56s). The
three current CPU receipts retain their actual publication and acknowledged
Show identities.
The normal Host product build, two independent boot checks, full journey and
final retained-product verification all exit zero on this accepted source.
The ISO SHA256 is `24aff22b80befdf3a8e0bf655b87e76ae3f8bd73f8eccae9b99bf957243e1eb5` and the kernel SHA256 is
`581ef493bc9f1a675333e2f7e2e9e806b89f8c03ae0857711288585ba6effb56`. All 17 current 1280×800 captures are healthy. The
paired receipt retains 27 available native Shows with 27 distinct identities,
one Birth, four accepted lifecycle actions, one refused action and fresh
recovery. The 640×480 probe of this exact ISO again completes acknowledged
Details, paging and return-to-primary stages, with actual extent 640×480,
no failure and process stop zero.

The 27 actual current guest Faces are also retained as immutable fixtures in
`evidence/native-face-runtime-snapshots-20261018.json`. CPU measurement visits
17,588 frames at all three extents, 100%/200% and both densities, with zero
refusals, zero text clipping, valid painted hits and unchanged Face/action
contracts. Frames use at most 36 commands and reach the exact 192-byte text
boundary. Every original subject name and owner text reconstructs in Details.
The focused runtime-face regression reuses the existing geometry assertions
and passes. Formatting and workspace Clippy with the same native/all-target
flags also pass after adding this regression (37.29s). Reusing captured Faces
is fixture evidence, not another owner
action, native reflow or enacted linear Show.

Home's outer shell cards may choose their own geometry; their screenshot is
not relabeled as the shared row policy. The captured current owner Faces are
measured directly, and their actions/relationships remain the owner contract.
The current normal product can be reproduced through:

```sh
cargo xtask make host build targets/conduitos/profiles/conduitos-native.host.conduit --output target/native-face-product
cargo xtask make host verify target/native-face-product --boot
cargo xtask make host verify target/native-face-product --journey
cargo xtask make host verify target/native-face-product
```


The current Source/Show wiring and its remaining browser, semantic-pointer and
installed stale-replay limitations are described by the upstream
[native shell cross-Mask audit](../proof/native-shell-cross-mask.md). Those
acceptance boundaries remain separate from L1 geometry, #5607, #5608 and parent
#5357. The historical receipts above remain immutable and separately classified.

### Per-encounter primary continuation

The following rows summarize the source-pinned raw comparison; intervals are
half-open. Full primary/detail command boxes and identities are in the archive.

| Encounter | Viewport | Scale | Primary pages before → after | Content y before → after | Text boxes exceeding bounds before → after |
|---|---|---:|---:|---|---:|
| Crèche | 1280×800 | 100% | 4 → 4 | `[56, 744]` → `[56, 744]` | 0 → 0 |
| Crèche | 1280×800 | 200% | 6 → 6 | `[56, 744]` → `[64, 744]` | 12 → 0 |
| Crèche | 640×480 | 100% | 8 → 8 | `[56, 424]` → `[56, 424]` | 0 → 0 |
| Crèche | 640×480 | 200% | 17 → 17 | `[56, 424]` → `[56, 424]` | 51 → 0 |
| Crèche | 320×240 | 100% | 39 → 39 | `[56, 184]` → `[56, 184]` | 0 → 0 |
| Crèche | 320×240 | 200% | 129 → 138 | `[56, 184]` → `[84, 184]` | 387 → 0 |
| Home (historical fixture) | 1280×800 | 100% | 1 → 1 | `[56, 744]` → `[56, 744]` | 0 → 0 |
| Home (historical fixture) | 1280×800 | 200% | 1 → 1 | `[56, 744]` → `[64, 744]` | 2 → 0 |
| Home (historical fixture) | 640×480 | 100% | 1 → 1 | `[56, 424]` → `[56, 424]` | 0 → 0 |
| Home (historical fixture) | 640×480 | 200% | 2 → 2 | `[56, 424]` → `[56, 424]` | 6 → 0 |
| Home (historical fixture) | 320×240 | 100% | 3 → 3 | `[56, 184]` → `[56, 184]` | 0 → 0 |
| Home (historical fixture) | 320×240 | 200% | 9 → 11 | `[56, 184]` → `[84, 184]` | 27 → 0 |
| Body (lulled) | 1280×800 | 100% | 1 → 1 | `[56, 744]` → `[56, 744]` | 0 → 0 |
| Body (lulled) | 1280×800 | 200% | 1 → 1 | `[56, 744]` → `[64, 744]` | 2 → 0 |
| Body (lulled) | 640×480 | 100% | 1 → 1 | `[56, 424]` → `[56, 424]` | 0 → 0 |
| Body (lulled) | 640×480 | 200% | 1 → 2 | `[56, 424]` → `[56, 424]` | 3 → 0 |
| Body (lulled) | 320×240 | 100% | 2 → 3 | `[56, 184]` → `[56, 184]` | 0 → 0 |
| Body (lulled) | 320×240 | 200% | 4 → 9 | `[56, 184]` → `[84, 184]` | 12 → 0 |
