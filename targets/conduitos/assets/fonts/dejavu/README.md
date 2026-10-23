# Pinned native graphical typography sources

Unmodified DejaVu Sans, DejaVu Serif and DejaVu Sans Mono from upstream release
**2.37** provide proportional interface text, editorial titles and exact code. These are
make inputs, not ambient host fonts. The complete upstream license is
preserved in `LICENSE`. Redistribution retains its copyright, trademark and
permission notices. The license permits inclusion in a larger package, excludes
selling the typeface by itself, and requires renamed modified fonts to avoid
reserved Bitstream/Vera names; see the complete retained terms for all conditions.
The added Serif source is unmodified.

Source: <https://github.com/dejavu-fonts/dejavu-fonts/releases/tag/version_2_37>

Archive: `dejavu-fonts-ttf-2.37.tar.bz2`, SHA-256
`fa9ca4d13871dd122f61258a80d01751d603b4d3ee14095d65453b4e846e17d7`.

| Unmodified asset | SHA-256 |
| --- | --- |
| `DejaVuSans.ttf` | `7da195a74c55bef988d0d48f9508bd5d849425c1770dba5d7bfc6ce9ed848954` |
| `DejaVuSerif.ttf` | `42d1edeb7952f31b1f96d767ed7030b08a39e0c372b0071641518864e2bffb51` |
| `DejaVuSansMono.ttf` | `b4a6c3e4faab8773f4ff761d56451646409f29abedd68f05d38c2df667d3c582` |

`native-ensemble.json` records the pinned sources and immutable mechanism mapping.
Make verifies checksums and prepares six fixed profiles: Sans14 label, Sans16
body, Sans20 heading, Serif24 editorial title, Mono14 code and Sans24 engineering
title. The admitted finite typography intent selects the title resource;
palette/identity does not select a font. Five semantic purposes remain unchanged.
The active path is `build.rs` (native-compositor) → `build/typography.rs` →
Cargo `OUT_DIR/native_typography.rs` and `native_coverage.bin`; runtime includes
those exact tables. `native_typography_resources.tsv` records each generated
profile's glyph/fallback counts, coverage bytes and metrics. Checked-in
`assets/graphical/manifest.json` is historical four-atlas provenance and is not
an input to this generator. Each input font is limited to 1 MiB; the repertoire
to 1,024 scalars; each raster edge to 64 pixels; and total grayscale coverage to
4 MiB. The repertoire includes the pinned Unifont subset and the shared Crèche
naming catalog with uppercase characters. Missing selected-front scalars use the
pinned Sans front, with the Mono advance retained for code. Remaining missing scalars use Unifont;
unsupported scalars use its explicit replacement glyph. Vertical metrics include
selected, Sans-fallback and rescue ink, with a rounding guard for admitted scale.

The native compositor samples the immutable coverage tables and blends against
retained surface pixels. No font parser or growing atlas runs during frames.
The layout cursor admits actual scaled ink bearings and extents as well as
advances, moving a following zero-advance mark with its base within one admitted
command. This is scalar handling, not full grapheme shaping: positive-advance
marks and the pre-existing UTF-8 item-to-command split are not generalized.
Unrenderable clusters at a given width refuse before paint. Chrome uses the
interface Label purpose; preferred density spacing may approach its existing
8-pixel minimum under pressure while preserving admitted text scale. Editor gaps
are measured around caption/text line requirements rather than assuming an old
font height. The layout cursor shares wrapping and height measurement, with input work
limited by `MAX_GRAPHICS_TEXT_BYTES`. Graphical text roles carry purpose, not
font names or sizes; canonical source obtains Code from its existing CodeBlock
component. The graphics wire encoding is version 2 and rejects old encodings
rather than interpreting their payload with changed offsets.

Historical #3163 proofs cover naming coverage, unsupported scalars, clipping, wrapping,
640 allocation-free text frames, and partial compositor damage. A native QEMU
journey has shown proportional prose and monospace source. The four Chromium
face-nucleus cases pass against the rebuilt WASM runtime, including
the role byte. These historical statements do not establish the new Serif's product, headless
or integrated acceptance. Fresh #5992 proof must retain exact Source/generated
tables and demonstrate normal product and headless exclusion separately. The
rescue renderer remains separate.
