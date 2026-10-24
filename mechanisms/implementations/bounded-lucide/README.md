# Bounded canonical palette icons

Conduit's primary palette icon source is [Lucide](https://lucide.dev/), pinned
to release `1.31.0` at upstream commit
`b7b6ecf1316d0af64c97a6b0392abe5e816a8e30`.

The bounded SVG subset under `svg/` includes the ten canonical `PaletteIconKey`
icons and the reviewed GUI navigation, action, status and subject icons used by
Masks. New GUI icons are copied unchanged from `icons/<name>.svg` at the same
pinned upstream commit. The retired native Patchbay shell was the only generated
Rust-table consumer. The bounded licensed source corpus remains reusable
realization material rather than Patchbay product meaning; current Masks may
consume the reviewed SVG set through their own target-owned path.
There is no runtime network or complete-pack dependency.

Lucide is distributed under the ISC license. A small set of Lucide icons is
derived from Feather and additionally carries the MIT notice; both notices are
preserved in [LICENSE](LICENSE).
