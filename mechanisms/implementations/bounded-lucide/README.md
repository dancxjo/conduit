# Bounded canonical palette icons

Conduit's primary palette icon source is [Lucide](https://lucide.dev/), pinned
to release `1.31.0` at upstream commit
`b7b6ecf1316d0af64c97a6b0392abe5e816a8e30`.

Only the ten SVGs named by the canonical `PaletteIconKey` table are retained
under `svg/`. The repository-development entrance

```console
cargo xtask make palette-icons mechanisms/implementations/bounded-lucide/svg products/patchbay/native/src/palette_icon_data.rs
```

validates that exact bounded set and deterministically rasterizes it into the
checked-in 16 by 16 monochrome masks consumed by the native Patchbay. The
bounded licensed source corpus is reusable realization material rather than
Patchbay product meaning. Other renderers consume the same semantic icon key
and may use the retained SVG.
There is no runtime network or complete-pack dependency.

Lucide is distributed under the ISC license. A small set of Lucide icons is
derived from Feather and additionally carries the MIT notice; both notices are
preserved in [LICENSE](LICENSE).
