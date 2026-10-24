# Actual authoring catalog and workspace consistency

Production Source: `7e16ac058`.

The real authoring catalog supplies 82 Kinds. The transport already admits up to
128, but the Library allowed five chunks of 16 (80), throwing before the graph
updated. The retained API summary shows nine authoritative Gear subjects while
the browser console shows `shared gear-results capacity exceeded`. The Library
now allows eight chunks, retaining 16-entry per-manifestation bounds and the
existing 128-entry transport limit. Only the required chunks are instantiated.

Workspace buttons previously allowed concurrent Save/Add/Frame operations. The
real workflow lost its named Teaching layout and notes. Actions now serialize
until validation/storage finishes, expose aria-busy, and disable all workspace
actions while pending. Refusals still show their actual message; controls regain
availability after completion. No domain edit, Face mutation or new wire format
is introduced.

## Positive local proof

- `browser-13.log`: one unbodied authoring regression plus all 12 HTML/furniture
  checks pass, with one Chromium worker and no retries. The authoring regression
  verifies the actual catalog exceeds 80 entries, all nine Gears render, an
  invalid exact integer refuses, and source identity remains unchanged. It holds
  actual workspace validation pending, verifies busy and disabled actions, then
  completes two layouts/notes/frames, explicit import, reload and correlation.
  The complete canonical Face compares equal before/after workspace operations;
  source bytes also compare equal. The default suite still retains the full
  lifecycle variant; this independent unbodied variant does not replace it.
- `simple-lifecycle.log`: the actual browser scenario authors/configures three
  Gears, connects/reroutes/removes Cords and a Gear, saves canonical Source,
  then completes Birth/Wake/Plan/Play. It passes in 50.5 seconds. Its bounded test
  deadline now matches the other authoring scenario at 90 seconds; no claim of
  satisfying the previous 45-second deadline is made.
- `contracts.log`: three exact configuration-value and authoring-basis checks
  pass. These preserve 64-bit decimal text and typed unit/quantity edits.

The selection helper now uses visible Close Library/Inspect/Related subjects
controls. Opening a disclosure inside a hidden inspector had produced false
selection timeouts. The interaction helper also observes request delivery
failure directly, and does not read an unused CDP response body after delivery.

The screenshot is the actual nine-Gear authoring view with an exact out-of-range
configuration refusal. It is local product evidence, not an illustration or a
whole-UI appearance/accessibility certification. Current executable/browser
assets use the explicit existing sealed WASM recorded in runtime-provenance.json.
No new WASM build, installed-owner, native QEMU or CI acceptance is claimed.

## Remaining negative evidence

The complex nine-Gear lifecycle variant now gets through its original catalog,
configuration, dual-layout and reload checks, but later fails with
`net::ERR_EMPTY_RESPONSE` during Play delivery. That failure remains unclassified
and unaccepted in complex-lifecycle-negative.log. The default authoring suite is
not claimed green. Earlier missing-layout and hidden-selection results remain
beside the positive checks; they are not substituted for lifecycle acceptance.

Checked Todo/Patchbay component Look selection, current native enactment,
compositor-custody acceptance and the full original parent scope remain open.
