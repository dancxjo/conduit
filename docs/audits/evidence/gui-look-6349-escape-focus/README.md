# Patchbay focused-surface dismissal

Source: `4afc3a901` (same production/test code as the exercised build, with one blank line removed).

Opening Library and then the Body summary leaves both Library and Inspect open.
Previously Escape from Library's search closed Inspect instead. The retained
before.log reproduces that failure against the preceding executable. The new
handler chooses the open surface containing the event target, returns focus to
its launcher, and retains the existing contextual fallback outside either panel.
Already handled keys and composition events are left to their owning controls.

A fresh local debug executable was built. All six tests in
`patchbay-panel-furniture.spec.mjs` pass with one Chromium worker and no retries,
including narrow forced-color/enlarged-text furniture coverage and the new real
UI regression. The new test checks both surfaces and both focus-return targets.
The screenshot shows Inspect's focused control after Library dismissal. This is
local documentary-fixture browser evidence, not owner or native Look enactment.

Command: `node proof/browser/node_modules/@playwright/test/cli.js test --config proof/browser/patchbay-html.playwright.config.mjs patchbay-panel-furniture.spec.mjs`

The graph Look path remains separate: current ComponentRegionInfo limits a
region to 20 subjects while Face admits up to 1,024. Reusing Todo's region bounds
as the graph model would incorrectly narrow the accepted Face population.
No graph component selection or new renderer contract is claimed here.
