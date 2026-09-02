# Patchbay renderer theme contract

Patchbay renderers share the toolkit-independent `PatchbayTheme` token contract from
`patchbay-model`. The `conduit.patchbay/phosphor@1` theme supplies bounded RGB values for
background, surface, structure, readable text, emphasis, focus, warning, failure, success, and
muted presentation roles. It contains no renderer primitives, fonts, geometry, effects, host
facts, or semantic identities.

The native renderer maps those roles directly to its finite pixel target. The HTML renderer
serializes the same Rust values into a bounded same-origin stylesheet and maps them through CSS
custom properties. The HTML renderer does not copy native drawing operations, and neither host
imports proprietary reference artwork.

Selection and keyboard focus use the yellow-gold focus role plus non-color cues: native selected
rows have a textual `>` marker, HTML selected graph nodes retain their selected class and stronger
stroke, and HTML keyboard focus retains a visible outline. Text remains legible without glow or
animation. A high-contrast browser option changes presentation only.

The public Conduit home, Book, Crèche, and browser Patchbay share the same shell vocabulary and
phosphor cyan/blue/amber roles. Dark is the reference palette; browser surfaces provide a restrained
light realization through the user's color-scheme preference, preserving the same role distinctions,
focus cues, and semantic behavior. The shell carries the same Home, Book, Crèche, and Source routes;
application workflow controls remain separate from those global destinations.

Theme values are decorative inputs after semantic planning. They do not participate in Form,
Body, Wake, Plan, Play, Host, Line, Sign, renderer-plan, or presentation identity. Tests verify
the exact shared mapping, native finite clipping, browser computed colors and focus/selection
cues, identity stability across theme changes, minimum contrast, and a deuteranopia simulation.

This contract does not introduce a widget system, renderer rewrite, CRT effect, layout change,
or semantic behavior.
