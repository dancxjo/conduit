# Browser Patchbay workbench Mask

These bounded modules realize Patchbay graph and navigation truth in a browser
workbench. They own browser geometry, routing, viewport interaction and
frontplate mechanics; they do not own the Forms, Gears, Ports, Cords, Plans,
Plays or Signs they depict.

The resident Patchbay workbench owns these realization modules. No standalone
Patchbay, Tour, Home, or Workspace HTML product is a semantic owner or an
alternate browser entrance.

`browser-sdk.js` is the deliberately small boundary for ordinary HTML. An
application obtains the immutable Body-owned topology with
`BrowserBody.patchbay()` from `@conduit/browser`, then gives that value to
`renderBrowserBodyPatchbay`. The workbench does not import Rust Patchbay crates,
raw Wasm exports, or Browser Host implementation modules.

`body-plan-inspection.js` accepts only the immutable value returned by
`BrowserBodyPreparation.inspection()`. It is read-only Mask interpretation: it
neither imports a Host implementation nor plans, claims a Play, or treats a
retained selection as current availability or physical proof.

`svg-viewport.js` adds bounded fit, zoom, and pan interaction to an SVG produced
by the resident lightweight Mask. It owns browser manifestation mechanics only;
the SVG remains the exact checked Form projection and the module cannot edit,
plan, or play it.
