# Canonical palette icons

Conduit's primary palette icon source is [Lucide](https://lucide.dev/), pinned
to release `1.31.0` at upstream commit
`b7b6ecf1316d0af64c97a6b0392abe5e816a8e30`.

Only the nine SVGs named by the canonical `PaletteIconKey` table are retained
under `svg/`. The repository-development entrance

```console
cargo xtask palette-icons products/patchbay/native/assets/icons/lucide/svg products/patchbay/native/src/palette_icon_data.rs
```

validates that exact bounded set and deterministically rasterizes it into the
checked-in 16 by 16 monochrome masks consumed by the native Patchbay. Other
renderers consume the same semantic icon key and may use the retained SVG.
There is no runtime network or complete-pack dependency.

Lucide is distributed under the ISC license. A small set of Lucide icons is
derived from Feather and additionally carries the MIT notice; both notices are
preserved in [LICENSE](LICENSE).
