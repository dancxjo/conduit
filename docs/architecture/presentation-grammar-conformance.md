# Universal Face grammar conformance

Face is Conduit's universal semantic waist for human encounter. It is
not a universal description of finished interfaces, and it does not promise
that an arbitrary existing interface can be encoded and reconstructed
identically.

The narrower claim is still demanding: truth intended for human encounter can
carry enough semantic and rhetorical structure that materially different Masks
can produce faithful, usable encounters without acquiring semantic authority.
This document is a pressure-test method for that claim. It is not a catalog of
standard interfaces and does not make their furniture canonical.

## Five boundaries

Classify every candidate fact before extending the grammar:

| Boundary | Owns |
| --- | --- |
| Plot and domain truth | what exists, remains true, and may happen |
| Projection | which of that truth matters in this encounter |
| Semantic composition | how the projected meanings rhetorically stand together |
| Mask realization | medium, interaction techniques, adaptation, and design |
| Show | the actual bounded encounter produced by that realization |

For example, two filesystem locations and their contents are domain truth;
that both locations matter now is projection; that they are being contrasted is
semantic composition; two side-by-side panes are one graphical Mask's
realization; and the particular rendered frame is a Show.

Do not move execution law into Face merely because it affects an
encounter. Choice, concurrency, synchronization, interruption, iteration, and
resume normally remain Plot truth. Face states their current humanly
relevant consequences: an action is available or refused, one subject follows
another, or a current activity has been interrupted. A proposal for richer
encounter structure must say why current actions, relationships, order, and
wording cannot express the meaning without importing application control.

## Admission questions

Before adding or changing a grammar construct, answer all of these:

1. What human meaning is lost without it?
2. Is that meaning authoritative domain truth, current projection, or rhetoric
   among projected truths?
3. Can a graphical, spoken, and deterministic-linear Mask preserve it without
   application-specific rewiring?
4. Can an unfamiliar Mask preserve the exact semantic identity and fall back
   honestly when it does not understand the domain?
5. Does the proposal name a medium, geometry, toolkit object, interaction
   technique, design system, or output artifact? If so, it belongs downstream.
6. Does it duplicate an existing subject, relationship, property, content,
   wording, action, input, disclosure, order, time, context, or composition
   statement? If so, collapse it rather than adding a synonym.
7. Does it prescribe application behavior across time rather than describe the
   present encounter? If so, it belongs in the Plot.
8. Is the value finite, exact, provenance-bearing, and safe to correlate back to
   the Face that authorized interaction?

Semantic composition is admitted sparingly. `contrast` and `emphasize` can
change what is communicated across media. `two-column`, `bold`, `window`, and
`spoken-first` prescribe realizations. The former may belong in Face;
the latter do not.

A rhetorical primitive is admissible only when removing it can change what a
human reasonably understands from the encounter across more than one medium.
That is stricter than proving that two renderers can implement it.

## Pressure-test corpus

Use the following archetypes to discover omissions, not to populate a closed
enum. For each specimen, record domain truth, projected truth, rhetorical
composition, Mask-only decisions, Plot-only interaction law, and what graphical,
spoken, and linear realizations must preserve.

| Archetype | Meaning that must survive | Tempting but non-universal furniture |
| --- | --- | --- |
| authored talk | progression, emphasis, reveal, comparison, supporting evidence | slide, title box, transition animation |
| program collection | programs, groups, names, availability, invocation | icon grid, overlapping windows |
| two-location file work | locations, membership, focus, selection, comparison, operations | left/right panes, function-key bar |
| spreadsheet | cells as domain subjects, formulas, dependencies, selection, edit actions | rows of pixels, column widths, formula bar |
| digital audio workstation | sources, tracks, temporal relationships, routing, current transport state | mixer strip, timeline pixels, fader |
| map | places, routes, containment, proximity when semantically asserted, current focus | tiles, zoom widget, screen coordinates |
| scientific plot | quantities, units, series, observations, uncertainty, comparison | axes in pixels, line color, legend box |
| book or article | sections, sequence, quotations, references, emphasis, subordinate matter | page size, columns, font metrics |
| command shell | current context, command intent, results, streams, terminal outcomes | prompt glyph, terminal escape sequence |
| nonvisual reader | the same names, structure, state, relationships, actions, and rhetoric | ARIA node, voice choice, braille cell |
| smart appliance | state, bounded values, commands, dependencies, refusal reasons | dial, touchscreen panel, voice phrase |
| collaborative classroom | participants, rooms, focus, activities, progress, authority differences | teacher dashboard, breakout-room tile |
| Patchbay | Plots, Gears, Ports, Cords, exact plan/play truth, source/destination roles | canvas, jack placement, zoom, cord curve |

The corpus is deliberately heterogeneous. Passing it does not prove
universality. A failure is useful: amend the grammar when a missing meaning is
genuinely cross-medium, collapse constructs when examples reveal duplication,
and leave a construct out when it merely reproduces a familiar interface.

For every specimen, compare semantic reconstruction rather than artifacts:

```text
authoritative specimen -> proposed Face -> several Masks
                                              -> what can the human understand?
                                              -> what can the human now do?
```

Ask those questions independently of each Show, then compare the answers. The
success condition is preservation of humanly relevant semantics, not visual or
verbal similarity. Same appearance does not establish same meaning; different
appearance does not establish different meaning.

The waist has two symmetric failure modes. Downward gluttony absorbs CSS,
panes, platform accessibility nodes, typography, and pixels. Upward gluttony
absorbs task models, state machines, scheduling, and application execution law.
Every conformance record must show that the proposed construct avoids both.

## Mask realization is planned design

Semantic preservation is necessary and insufficient for design quality. A Mask
may treat realization as an ordinary planned problem over the Face,
medium, available implementations and resources, context, policy, user needs,
conventions, and aesthetic knowledge. It need not be a fixed table from role to
widget.

That freedom grants no semantic authority. A Mask may select typography,
layout, speech structure, tactile grouping, interaction techniques, or a design
system; it may not invent facts, actions, relationships, availability, or
authority. Hand-authored and adaptive design remain possible downstream of the
waist so long as the resulting Show is correlated with the exact Face
and interactions return through its bounded semantic contract.

## Proof record

A grammar change should include at least one bounded specimen that failed
before the change and passes afterward. Record:

- the missing human meaning rather than the desired visual effect;
- why existing constructs could not express it;
- which boundaries own the new statement and its realization;
- graphical, spoken, and linear preservation evidence;
- finite bounds, identity, provenance, and stale-interaction behavior; and
- a negative test showing that nearby medium-specific machinery remains outside
  Face.

The grammar earns the word *universal* by surviving varied encounters while
remaining narrow, not by accumulating the nouns of every interface examined.

The current conformance fixtures and Rust crate still use `presentation` and
`Presentation` as migration-era serialized and implementation names. They
exercise Face; they do not establish a Presentation layer between Face and
Mask.
