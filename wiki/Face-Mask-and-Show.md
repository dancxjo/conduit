# Face, Mask and Show

Conduit's human-interface architecture separates **what a person should be able to understand or do** from **how a particular medium realizes that encounter**.

```text
authoritative truth
      ↓
face
      ↓
mask
      ↓
show
```

## Face

A **face** is the body's renderer-neutral semantic encounter.

It can carry things such as:

- subjects and relationships;
- typed content;
- wording;
- actions and inputs;
- semantic order;
- context and provenance;
- what is currently relevant or refused.

Face is not a widget tree. It does not mean HTML, windows, panes, pixels, speech timing, or terminal escape sequences.

## Mask

A **mask** is an ordinary form serving the user-agent realization role.

Current tree graphical mask:

```conduit
form native-graphical (
    >> face: Presentation
    interaction: FaceInteraction...| >>
    show: Show >>
) {
    spread: presentation/tee
    render: presentation/renderer
    human: face/interaction

    face >> spread.presentation
    spread.presentation >> render.presentation
    spread.presentation >> human.presentation
    render.show >> human.show
    render.show >> show
    human.interaction >> interaction
}
```

The spoken mask has the same semantic role but may be realized through speech/audio machinery.

There is no special `mask` declaration. Mask is a role played by an ordinary checked form.

## Show

A **show** is one finite realized occurrence of a face through a mask.

Examples:

- a browser DOM/SVG encounter;
- a native window;
- a ConduitOS surface;
- speech;
- deterministic linear text;
- another admitted medium.

Shows need not look or sound alike.

## Same meaning does not mean same furniture

The Face conformance work deliberately attacks familiar interfaces across radically different media.

A meaningful cross-medium property may belong in Face:

- emphasis when it changes understanding;
- semantic contrast;
- ordered traversal;
- source/destination roles;
- independent exploration when it changes what can be done.

Medium furniture belongs downstream:

- red;
- 48pt;
- left pane;
- two columns;
- fly-in animation.

The test is not "can every UI be serialized?" It is "can the humanly relevant meaning and agency survive realization?"

See [#4167](https://github.com/dancxjo/conduit/issues/4167).

## Wardrobe

A body can author mask policy:

```conduit
with masks/native-graphical as graphical
with masks/spoken as spoken

body roseau {
    wear graphical else spoken
    want graphical over spoken
}
```

`wear graphical else spoken` puts the fallback structure into the plan.

`want` is policy among semantically eligible alternatives. It cannot make an ineligible mask valid.

Runtime `wear` and `doff` request wardrobe change and therefore planning where required. They do not mutate an immutable plan.

## Direct and recursive masks are peers

A browser might realize a graphical mask directly at a high semantic seam.

ConduitOS might realize the same role recursively:

```text
face
 -> graphical mask
 -> layout/compositor
 -> raster
 -> scanout
 -> show
```

Both can be honest realizations if they preserve the mask contract. Conduit does not force every host through framebuffer-shaped machinery.

## Generative masks are not semantic authority

A model can participate in realization without becoming the application.

The important direction is:

```text
exact face truth
 -> generative interpretation
 -> validation against exact semantic constraints
 -> show
```

Model prose does not create actions, authority, state, or domain truth by eloquence alone.

The closed LLM mask issues built validation and correlation precisely to keep generative presentation expressive without giving it application authority.

## Residue is not current truth

A show can physically outlive the face revision that produced it.

A stale frame, retained DOM, buffered audio file, printed page, or transcript is **residue**, not proof of the current face/mask state.

This matters especially during replan and recovery.

## Patchbay is an ordinary participant

Patchbay is not the definition of Face. It is an inspection/workbench application whose own encounter should obey the same Face/Mask/Show rules as everything else.

That is an architectural pressure test: the debugger should not require a second UI ontology.
