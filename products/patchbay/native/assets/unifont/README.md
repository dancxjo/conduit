# Patchbay GNU Unifont subset

`unifont-17.0.04-patchbay.hex` is a mechanically extracted, bounded subset of
GNU Unifont 17.0.04. It contains ASCII, Latin-1, Greek and Coptic, Cyrillic,
the em dash used by renderer-neutral titled text, arrows, box drawing,
geometric shapes, one double-width CJK demonstration glyph (`U+4E2D`), and
the explicit replacement glyph (`U+FFFD`). It also includes every printable
character in the shared Crèche naming catalog, selected directly from
`products/creche/names/catalog.mjs`. The resulting 971 glyphs remain within the
existing 1,024-glyph bound. Glyph coverage alone does not establish combining
mark placement or general text shaping; those remain renderer obligations.

Upstream source:

- <https://unifoundry.com/pub/unifont/unifont-17.0.04/font-builds/unifont_all-17.0.04.hex.gz>
- SHA-256: `c31d210962408a00de8e2ebe2f2fc26824d7a4939d4eb15d347761fb2a0b39a6`
- package source SHA-256: `5c52c5d56ef98089ddbca62e68560ceccc57ea88940b9d38cc3c888fe3b59a34`
- generated subset SHA-256: `08701b3d465956a53b599fca8adbc6eb18b77ee1681daca4eb70b2b7b509d26d`

Regenerate from the downloaded, checksum-verified asset:

```sh
cargo xtask unifont-subset \
  unifont_all-17.0.04.hex.gz \
  products/patchbay/native/assets/unifont/unifont-17.0.04-patchbay.hex
```

Upstream dual-licenses its font glyphs under SIL Open Font License 1.1 and
GPL-2.0-or-later with the GNU font embedding exception, apart from two named
Jiskan source files that are public domain. The exact package notice and full
license texts are preserved in `COPYING`; `OFL-1.1.txt` is also retained as a
standalone copy. GNU Unifont was created by Roman Czyborra and has been
developed by the GNU Unifont contributors, with releases assembled by Paul
Hardy. This subset changes only coverage, not glyph bitmaps, and uses the
upstream font name only for attribution.
