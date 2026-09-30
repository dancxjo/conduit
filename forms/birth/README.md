# Birth draft

`conduit-birth-form` owns the renderer-neutral Birth draft: bounded friendly-name
editing and suggestion, naming tradition and variation, revision-checked form
selection, search state, and the exact reviewed `BirthSelection`. It is a
`no_std` crate using `alloc`; it consumes `conduit-body`'s existing `ResidentForm` and
`BodyWorkset` contracts without creating a body, granting authority, planning,
or starting a play. The owning lifecycle boundary still reviews and accepts
that selection.

The [shared naming corpus](names/README.md) remains one versioned source for
Rust and the browser. Moving ownership does not change the catalog bytes,
SHA-256 domain, seed encoding, suggestion order, or UTF-8 bounds.

The live widget view and event protocol remain explicit consumers in
`products/creche/model`: `BirthPresentation` and `BirthActions` extend this
draft for the current browser and ConduitOS entrances. Their control vocabulary,
action identities, and projection are not part of this crate. Existing refusal
spellings, including `StalePresentation`, remain unchanged in this extraction.
This is the semantic extraction seam for #4231, not retirement of those live
adapters or a new body lifecycle.

Renderer-neutral draft and naming contracts are tested here. Widget lowering,
event routing, and view bounds stay with the Crèche adapter; browser and native
lifecycle integration stays with those consumers. The repository workspace
check (`cargo xtask check`) includes this package in the foundation test shard.
