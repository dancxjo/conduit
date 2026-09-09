# Pinned native graphical typography sources

Unmodified DejaVu Sans and DejaVu Sans Mono from upstream release **2.37**
are the proportional UI and monospace code sources for #3163. These are
fabrication inputs, not ambient Host fonts or a claim of completed runtime
integration. The complete upstream license is preserved in `LICENSE`.

Source: <https://github.com/dejavu-fonts/dejavu-fonts/releases/tag/version_2_37>

Archive: `dejavu-fonts-ttf-2.37.tar.bz2`, SHA-256
`fa9ca4d13871dd122f61258a80d01751d603b4d3ee14095d65453b4e846e17d7`.

| Unmodified asset | SHA-256 |
| --- | --- |
| `DejaVuSans.ttf` | `7da195a74c55bef988d0d48f9508bd5d849425c1770dba5d7bfc6ce9ed848954` |
| `DejaVuSansMono.ttf` | `b4a6c3e4faab8773f4ff761d56451646409f29abedd68f05d38c2df667d3c582` |

The graphical profile must admit a fixed set of sizes, glyphs, decoded
coverage bytes, and metrics before frames use them. Native text roles choose
among those admitted metrics below Presentation meaning. Headless profiles
must not include these font bytes or their raster machinery. Existing Unifont
coverage remains a separate deterministic fallback/rescue resource; merely
checking these assets in does not satisfy those runtime and proof obligations.
