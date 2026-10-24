# Measured Patchbay status footer

Production Source: `a74afe3f5`.

Current truth, temporal context, run controls and local workspace tools formerly
floated over graph nodes and over each other. They now share a bounded footer
with native scrolling and a keyboard focus target. Its measured height reserves
graph space, including when tools or provenance are expanded. The source order,
labels, action handlers and disclosures remain intact. Desktop navigation uses
the available width instead of compressing its controls into a right-hand block.

The exact omitted-Cord notice and live status role now render into an explicit
footer target through the existing React tree. Other Flow consumers without a
target retain their existing notice. No copied Face model, inferred graph facts
or semantic identity change is introduced. The 24,000-character HTML ceiling
is retained by removing redundant XML namespace declarations from inline HTML
SVG elements; browser SVG rendering remains intact.

## Local proof

The retained before.log reproduces overlap on the preceding executable. A fresh
local executable passes all 12 checks in `patchbay-html.spec.mjs` and
`patchbay-panel-furniture.spec.mjs`, one Chromium worker, no retries. These cover
exact Face/Plan/Play/subject identity through Flow mechanics, typed interaction,
delivery loss, graph reconciliation, drawer focus return, measured graph space,
all footer sections outside the graph, actual workspace-field focus, 390px with
32px root text, forced colors and reduced motion. The screenshot is the actual
documentary-fixture Plot scene after Fit. No image compositing is used.

The old proof counted graph area underneath controls as full-window canvas;
it now requires the graph to fill the measured space between chrome bands with
no band overlap. The legacy root theme-variable assertion now checks the
canonical Look identity at its actual scope. These changes preserve content and
interaction checks rather than treating hidden canvas area as usable space.

Command:
`node proof/browser/node_modules/@playwright/test/cli.js test --config proof/browser/patchbay-html.playwright.config.mjs patchbay-panel-furniture.spec.mjs patchbay-html.spec.mjs`

## Negative results and remaining acceptance

The first footer run exceeded the existing HTML ceiling by 69 characters; the
retained size refusal precedes the namespace cleanup. A broader authoring run
initially refused the missing default runtime path. Retrying with the existing
sealed runtime (runtime-provenance.json) reached the UI but both scenarios failed:
one found zero Gear nodes after opening its Plot; the other attempted selection
inside a hidden inspector. These failures are retained, not classified as fixed,
not assigned to an unverified cause and not counted as authoring acceptance.

This is local documentary-fixture browser proof. It does not establish actual
owner/native Look switching, native QEMU, whole-GUI accessibility certification,
CI/integration/publication or original parent acceptance. Checked cross-component
Look selection and the native compositor-custody refusal remain open.
