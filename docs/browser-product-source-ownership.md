# Browser product source ownership

Issue #2277 owns this source boundary. Exact release acceptance belongs in STATUS.md and the issue.

Tour browser source, styles, routing, state and package descriptor live in `products/tour/browser/`; authored lessons remain in `products/tour/content/`. Crèche browser lifecycle, actions, routing, styles and descriptor live in `products/creche/browser/`. Patchbay keeps its existing browser manifestation and specialized graph renderer in `products/patchbay/html/`.

The browser Host owns package admission/loading, bounded generic presentation, identity, storage, membership and browser effects. Product modules import those owners through explicit dependency specifiers. Staging copies the selected dependency bytes into one finite package; the existing loader verifies every resource digest and replaces declared imports with verified module URLs. No source copy is retained under the Host and no product is routed through another product's package.

Target fabrication and browser deployment adapters remain in their existing target roots. Crèche consumes their reviewed contributions; moving its source does not transfer target policy or turn a presentation action into authority. Opening any product still does not implicitly birth a Body or start a Play.

Tour staging is `scripts/ci/stage-tour-product.sh`, used internally by the existing `cargo xtask demo tour` and CI entrances. Public `/tour/`, `/creche/` and `/patchbay/` routes are unchanged. The historical reading-state compatibility identity stays explicit so existing persisted state is not silently discarded; current module, style, action and presentation slot identities use Tour names.
