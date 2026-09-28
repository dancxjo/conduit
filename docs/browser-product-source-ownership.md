# Browser product source ownership

This is the current source boundary, introduced by #2277. See
[repository layout](repository-layout.md) for placement and [status](../STATUS.md)
for executable coverage.

Tour browser source, styles, routing, state and package descriptor live in `products/tour/browser/`; authored lessons remain in `products/tour/content/`. Crèche browser lifecycle, actions, routing, styles and descriptor live in `products/creche/browser/`. Patchbay keeps its existing browser manifestation and specialized graph renderer in `products/patchbay/html/`.

The browser host owns package admission/loading, bounded generic presentation, identity, storage, membership and browser effects. Product modules import those owners through explicit dependency specifiers. Staging copies the selected dependency bytes into one finite package; the existing loader verifies every resource digest and replaces declared imports with verified module URLs. No source copy is retained under the host and no product is routed through another product's package.

Target fabrication and browser deployment adapters remain in their existing target roots. Crèche consumes their reviewed contributions; moving its source does not transfer target policy or turn a presentation action into authority. Opening any product still does not implicitly birth a body or start a play.

Tour staging is `products/tour/tools/stage-tour-product.sh`, used internally by
the existing `cargo xtask demo tour` and CI proof entrances. Tour is no longer a
public Pages product and does not retain the retired Book route or saved-state
dialect. Its remaining development package owns one bounded
`conduit.application/tour-reading-state` identity while its lessons and proofs
are harvested into the Body-and-Face architecture.
