# Birth draft

`conduit-birth-plot` owns the renderer-neutral Birth draft: bounded friendly-name
editing and suggestion, naming tradition and variation, revision-checked plot
selection, search state, and the exact reviewed `BirthSelection`. It is a
`no_std` crate using `alloc`; it consumes `conduit-body`'s existing `ResidentPlot` and
`BodyWorkset` contracts without creating a body, granting authority, planning,
or starting a play. The owning lifecycle boundary still reviews and accepts
that selection.

The [shared naming corpus](names/README.md) remains one versioned source for
Rust and the browser. Moving ownership does not change the catalog bytes,
SHA-256 domain, seed encoding, suggestion order, or UTF-8 bounds.

`BirthPresentation` and `BirthActions` own the Plot's exact Face encounter and
extend this draft for the current browser and ConduitOS entrances. Their control
vocabulary, action identities, and projection remain one shared bounded
contract. Existing refusal spellings, including `StalePresentation`, remain
unchanged. This ownership does not create another body lifecycle or renderer.

Draft, naming, Face lowering, event routing, and view bounds are tested here;
browser and native lifecycle integration stays with those consumers. The
repository workspace check (`cargo xtask check`) includes this package in the
foundation test shard.
