# Native resolved paint contrast

Refs #5629 and #5994; delivery #6255.

The prior native profile check compares base text tokens only against the page
background. The new focused checks call the production compositor paint
resolver instead: both finite palettes, the no-expression default, admitted
forced colors, and an explicit Encoding refusal of the requested Look taking the trusted
Phosphor fallback. This exercises admission fallback, not asset lookup failure. They use both RGB/BGR display channel layouts and compare
resolved text and control paint against both actual Background and Surface.

Every text role (foreground, muted, accent, status, warning, success and danger)
must meet a 4.5:1 relative-luminance contrast ratio. Focus, hover, selection and
control border roles must meet 3:1. User constraints retain 200% text through
all requested/fallback combinations. The tests do not copy palette RGB values;
they exercise the colors returned by the actual resolver. Only a test-module
registration changes production paint source; no palette or rendering changes.

This is CPU evidence for resolved full-coverage paint tokens. Antialiased edge
pixels, actual typography/reflow and paint overlap, visual reading order,
physical platform colors, human usability and universal accessibility are not
proved by these ratios. The live owner override scenario remains required;
these checks do not close #5629 or #5994 or replace exact-head integration.

Both focused native-compositor checks pass (two cases, 1,332 filtered out).
Formatting and whitespace checks pass.
