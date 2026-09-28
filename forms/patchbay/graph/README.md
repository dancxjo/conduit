# Patchbay graph projection

This `no_std` package owns Patchbay's finite gear, port, cord, front-control,
and recursive form-boundary projections. It derives them from checked and
expanded form truth; it does not own execution, host offers, rendering, or
input-device state.

`patchbay-model` retains its existing public paths by re-exporting this
package. Hosted and native consumers can therefore share the same identities,
connection compatibility, inspection facts, and graph bounds.

Construction uses `alloc` during preparation. `no_std` does not mean
allocation-free or guarantee that a native allocator reclaims discarded
projections. Callers must admit storage and preserve the distinction between
preparation and play.

Renderer-local geometry remains outside the graph. A retained selection uses
`PatchbaySubjectRef`, including its exact expanded form identity, so a stale
hit target cannot silently select a subject in a replacement graph.

This extraction does not itself switch the ConduitOS Tour renderer to this
graph or establish a resident native Patchbay form. Those are downstream
integration steps.
