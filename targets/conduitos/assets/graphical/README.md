# Default native graphical profile

`conduitos/graphical/dejavu-v1` is the compiled profile for the canonical x86_64
live image. It uses DejaVu Sans 2.37 for proportional UI text and DejaVu Sans
Mono 2.37 for code. The four immutable coverage atlases have no runtime font
parser, discovery, GPU dependency, allocation, or glyph cache. The build rejects
missing, malformed, noncanonical, or oversized atlases. Headless products do not
select the native compositor and do not link these resources.

| Purpose | Face / pixels | Line advance |
| --- | --- | --- |
| Title | Sans 26 | 34 |
| Heading, action | Sans Bold 18 | 24 |
| Body, label, status, warning, supporting | Sans 14 | 18 |
| Code and exact identifiers | Mono 13 | 18 |

The admitted corpus is the pinned Unifont subset. Covered glyphs use the primary
face; remaining admitted characters (including the Chinese specimen) use the
same pinned Unifont bitmaps. Unadmitted characters deterministically use U+FFFD.
This is bounded glyph coverage, not general multilingual shaping. Exact UTF-8
content remains unchanged. Measurement and raster placement share advances and
word wrapping, including explicit newlines and long-token wrapping.

Spacing is limited to 4/8/12/24/32, with one-pixel borders and six-pixel corner
radii. Foreground, background, accent, warning, success, and supporting colors
are defined in `display/profile.rs`. Text tokens meet 4.5:1 contrast against the
background. Keyboard focus is an outline, hover changes the outlined cursor,
and selection retains its separate marked item. Icons redundantly accompany
labels rather than replacing them. Original fixed 16×16 mechanisms cover Body,
Wake, Plan, Play, Host, Gear, Port, Line, status, warning, close, back, and confirm.

Semantic text roles travel in the bounded graphics leaf, without font-family or
pixel facts in Presentation. Graphics encoding v2 carries one role byte per
command; the decoder accepts v1 as the body role. Shape and text identities and
canonical validation remain distinct. The primitive font path is retained for
non-compositor proof appliances; successful ordinary graphical boot emits the
profile receipt after its first rendered frame and cannot select that path.

Run `cargo xtask conduitos graphical-profile-proof` to fabricate the canonical
live ISO and drive its actual native journey. Its retained screenshots and
profile receipt are development/emulator evidence, not physical qualification
or stable-release acceptance. The existing release gallery carries the same
profile metadata and representative entry, Tour, selection, and Inspector
screenshots after promotion.

`manifest.json` records exact source font and atlas hashes, raster sizes, glyph
counts, and storage. `LICENSE.txt` retains the font redistribution license.
`rasterize.py` is the maintenance generator; it checks the three exact source
hashes and uses Pillow's basic FreeType layout. Ordinary builds consume the
checked-in atlas bytes and need neither Python nor installed system fonts.
The Unifont fallback license and provenance remain in
`products/patchbay/native/assets/unifont` at the repository root. No archived
subsystem was recovered for this profile.
