# Pinned native graphical typography sources

Unmodified DejaVu Sans and DejaVu Sans Mono from upstream release **2.37**
are the proportional UI and monospace code sources for #3163. These are
fabrication inputs, not ambient Host fonts. The complete upstream license is
preserved in `LICENSE`.

Source: <https://github.com/dejavu-fonts/dejavu-fonts/releases/tag/version_2_37>

Archive: `dejavu-fonts-ttf-2.37.tar.bz2`, SHA-256
`fa9ca4d13871dd122f61258a80d01751d603b4d3ee14095d65453b4e846e17d7`.

| Unmodified asset | SHA-256 |
| --- | --- |
| `DejaVuSans.ttf` | `7da195a74c55bef988d0d48f9508bd5d849425c1770dba5d7bfc6ce9ed848954` |
| `DejaVuSansMono.ttf` | `b4a6c3e4faab8773f4ff761d56451646409f29abedd68f05d38c2df667d3c582` |

Fabrication verifies these checksums and prepares five fixed profiles: label,
body, heading, title, and code. Each input font is limited to 1 MiB; the repertoire
to 1,024 scalars; each raster edge to 64 pixels; and total grayscale coverage to
4 MiB. The repertoire includes the pinned Unifont subset and the shared Crèche
naming catalog with uppercase forms. Missing code-face scalars use the pinned
UI face with the code advance retained. Remaining missing scalars use Unifont;
unsupported scalars use its explicit replacement glyph.

The native compositor samples the immutable coverage tables and blends against
retained surface pixels. No font parser or growing atlas runs during frames.
The layout cursor shares wrapping and height measurement, with input work
limited by `MAX_GRAPHICS_TEXT_BYTES`. Graphical text roles carry purpose, not
font names or sizes; canonical source obtains Code from its existing CodeBlock
component. The graphics wire encoding is version 2 and rejects old encodings
rather than interpreting their payload with changed offsets.

Focused proofs cover naming coverage, unsupported scalars, clipping, wrapping,
640 allocation-free text frames, and partial compositor damage. A native QEMU
journey has shown proportional prose and monospace source. Full browser proof,
headless final-artifact exclusion, and exact integrated acceptance remain
required before closing #3163. The rescue renderer remains separate.
