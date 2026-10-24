# Quantity Play runtime prerequisite

Tracks [#6387](https://github.com/dancxjo/conduit/issues/6387), discovered during #6349 GUI acceptance. This is negative evidence, not a runtime repair.

At source `0e525be01e3dec943daa3763a27635656b522be8` (production `c1e9fe399`), the locally rebuilt workbench exits 101 during the minimal four-Gear Quantity Play. The current Face/revision and actually available action IDs drive Open/Birth/Wake/Plan successfully. Play loses HTTP delivery and stderr reports `ready math output: OutputBlocked` at `targets/std/src/installed_std/math_backs.rs:75`.

`minimal-result.json` retains the complete Source, requests, semantic basis and stderr. `complex-browser.log` retains the original complex authoring test failure and newly captured server stderr. Both used the existing sealed browser WASM from `350be7db7`, not a new runtime WASM build. The relevant core/kernel/planner/catalog/math source matched the fetched dev tree. No CI was dispatched.

The permanent checked queue bounds and temporary byte-pressure handling both require repair. Merely replacing the panic with Await would not prove delivery. The original full GUI result remains 14/15 passing; native and cross-component Look acceptance remain separate.
