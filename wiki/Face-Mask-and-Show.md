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

## The human interface has a semantic waist

Conduit does not try to invent one universal widget toolkit.

Instead, it asks for the smallest renderer-neutral layer rich enough to preserve **what the person is meant to understand and what agency the person is meant to have**.

That layer is the **face**.

Everything above the face is domain and body truth. Everything below it is realization for a medium, user agent, device, accessibility mode, or presentation technology.

This is the same architectural move used elsewhere: meaning first, exact realization second.

## face

A **face** is the body's renderer-neutral semantic encounter.

It can carry things such as:

- subjects and relationships;
- typed content;
- wording;
- actions and inputs;
- semantic order;
- context and provenance;
- what is currently relevant or refused.

face is not a widget tree. It does not mean HTML, windows, panes, pixels, speech timing, or terminal escape sequences.

## mask

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

There is no special `mask` declaration. mask is a role played by an ordinary checked form.

## show

A **show** is one finite realized occurrence of a face through a mask.

Examples:

- a browser DOM/SVG encounter;
- a native window;
- a ConduitOS surface;
- speech;
- deterministic linear text;
- another admitted medium.

shows need not look or sound alike.

## Semantic fidelity is not visual sameness

Two shows can differ radically while remaining faithful to the same face.

A graphical show may use spatial layout; a spoken show may use sequence and prosody; a terminal show may use terse text. The invariant is not pixel identity. The invariant is preservation of the face's semantic structure, relationships, actions, state, and relevant emphasis.

This makes accessibility and alternative media architectural peers rather than after-the-fact translations of a privileged screen.

## Same meaning does not mean same furniture

The face conformance work deliberately attacks familiar interfaces across radically different media.

A meaningful cross-medium property may belong in face:

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
    wear graphical, spoken
    want graphical over spoken
}
```

`wear graphical, spoken` permits either mask without ranking them. Only routes
already sealed into the current plan are candidates for same-plan selection.

`want` is optional policy among semantically eligible alternatives. It cannot make an ineligible mask valid.

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

Patchbay is not the definition of face. It is an inspection/workbench application whose own encounter should obey the same face/mask/show rules as everything else.

That is an architectural pressure test: the debugger should not require a second UI ontology.
