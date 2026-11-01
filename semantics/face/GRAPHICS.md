# Bounded graphical scene contract

A graphical scene carries resolved drawing obligations below semantic Face. It
contains no Face subjects, Look proposals, resource reservations or authority to
present a Show. The [codec](src/graphics.rs) and [closed forms](types.conduit)
own the accepted drawing vocabulary.

## Semantic identity and packet version

The combined vocabulary is `face/graphics-scene@2`. Graphics transforms declare
`conduit.std/graphics-scene@2`; the terminal declares
`conduit.std/face-graphics@2`. These revisions cover legacy commands together
with Polyline, Value text and explicit rectangle metrics, as one atomic domain.
A historical reader accepting only a subset cannot truthfully offer this domain.
Matching packet framing or artifact identity does not prove semantic compatibility.

Storage remains 48 commands, 192 payload bytes per command and 10,226 bytes per
encoded scene. This extension adds no queue, scene or glyph-storage allowance.
Look encoding remains independently version 2; the global protocol is unchanged.

A packet begins with version and command count. Each command has a 21-byte
header and its declared payload: command, paint and shape tags; bounds and clip
rectangles; payload length; text-role tag. Rectangles contain little-endian signed
16-bit coordinates and unsigned 16-bit extents. Bounds and clip must have positive
extents within the existing coordinate limits.

| Content | Canonical packet version |
| --- | --- |
| No explicit rectangle metrics, including Polyline and Value | 2 |
| At least one rectangle with explicit metrics, including radius zero | 3 |

Legacy-shaped commands retain their existing version-two bytes. Version two
with explicit metrics and version three without metrics both refuse. Only the
`encoded_len()` prefix of fixed-capacity encoding is a packet: trailing padding
is not accepted. An unsupported version, tag or malformed geometry must refuse.
Packet decoding is not a substitute for planning compatibility checks.

## Binary geometry

[Explicit rectangles](src/graphics/rectangle.rs) have a two-byte payload on
RoundedFill or RoundedStroke: radius 0..32, inset stroke 1..8. Radius zero is
explicitly square. Other styles cannot carry these metrics. Empty payloads retain
legacy geometry. Native rasterization clamps radius to half the shorter side;
a stroke covering the interior fills it. Clipping does not invent a border.

[Polyline](src/graphics/polyline.rs) appends command tag 5. It retains 2..16 exact
coordinate pairs, with no consecutive duplicate points. Diagonal segments are
allowed; their declared bounds must exactly enclose the points. OrthogonalPath
retains tag 4, its eight-point bound, and its orthogonality rule. Both require
Stroke. Native rendering uses bounded integer segments without interpolation of
measurements or temporal meaning.

`payload()` never interprets rectangle or path geometry as UTF-8. These command
kinds retain the Body text-role byte; any other role on a nontext command refuses.

## Text purpose

Value appends text-role tag 10. Existing roles keep their tags: Body=1, Title=2,
Heading=3, Label=4, Code=5, Status=6, Warning=7, Action=8, Muted=9. Value expresses
a readout's visual purpose; it does not select a semantic unit or change Face.

Native selected typography resolves Value to twice the Body token using the
existing Body glyph resource. Preparation must still fit admitted glyph and line
bounds. A syntactically valid Look is not a promise that every Host can realize it.

## Evidence boundaries

[Codec tests](src/graphics/rectangle_tests.rs) cover all admitted radius/stroke
pairs, canonical versions, legacy bytes and a combined Polyline/Value/rectangle
packet. Native rectangle and polyline tests cover raster pixels and clipping.
The [browser proof reader](../../proof/browser/graphics-scene-decoder.mjs) reads
binary geometry separately from text. Browser production transforms use the
shared Rust codec; the proof reader is not planning or owner authority.

The [planning matrix](../../targets/std/tests/graphics_revision_planning.rs)
checks actual portable contracts against installed offers at default and explicit
selection entrances for both checked and expanded canonical plots. It is a planning proof, not evidence of Host Calls, a live
owner, acknowledged Show, or physical device adoption. The integration owner's
borrowed planning entrance and real expanded-packet execution require separate
qualification before this slice is admitted.
