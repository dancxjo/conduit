# Browser Patchbay workbench Mask

These bounded modules realize Patchbay graph and navigation truth in a browser
workbench. They own browser geometry, routing, viewport interaction and
frontplate mechanics; they do not own the Forms, Gears, Ports, Cords, Plans,
Plays or Signs they depict.

The Patchbay and Tour browser packages stage the same modules at their local
resource paths. Keeping the realization with the resident Patchbay workbench
avoids making either legacy product shell the semantic owner.

`browser-sdk.js` is the deliberately small boundary for ordinary HTML. An
application obtains the immutable Body-owned topology with
`BrowserBody.patchbay()` from `@conduit/browser`, then gives that value to
`renderBrowserBodyPatchbay`. The workbench does not import Rust Patchbay crates,
raw Wasm exports, or Browser Host implementation modules.
