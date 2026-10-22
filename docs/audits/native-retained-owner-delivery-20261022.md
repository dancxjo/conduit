# Canonical publication on the retained native application owner

This #6201 slice reuses native-local1ef04728b's application-core delta only.
The actual selected Patchbay actor now supports immutable publication, read-only
fragment query and one-use typed consumption. It creates no second graph or
controller. Root custody/acknowledgement validation and production selection are
still separate integration requirements; Source and caller routes do not switch.

Canonical mode is one-way and preserves the consumed-Show watermark. Legacy
events and snapshot generation are excluded for canonical slots. Target/topology
changes revoke publication. Publication validates complete Face/MaskShow and
exact owner contribution; rejected replacement revokes old evidence rather than
reopening it. Consumption takes the publication before decoding or admission,
then preflights the exact prospective reply before mutating the retained actor.
Malformed/refused input consumes that publication once; actor-domain and typed
interaction refusals remain distinct from transport errors.

The existing staged State/codec path reserves actual reply capacity. The private
heap check shares the unchanged initializer and exercises genuine service
query/publication/consume/replay over its retained startup controllers. Canonical
rounded-live peak is1909952 bytes in the unchanged2072576-byte bitmap heap.
The existing large Graph/ProjectGraph scenario peaks at2042464, leaving30112
combined rounded-live headroom. Both states release all tracked allocations;
existing impossible-reservation and terminal-exhaustion checks still pass.
This does not guarantee fragmentation behavior, simultaneous scenarios or
ordinary native full-Mask/stack/private-service fit. The Show and Plan/Play are
explicit CPU model fixtures, not actual native acknowledgement evidence.

Four canonical/transport cases and the fixed-heap executable pass. Final owning
package all-target check, freestanding x86_64 library check and changed-Rust
formatting pass. Runtime dependencies/global limits remain unchanged; two dev
fixture dependencies are the sole lockfile additions. Logs/checksums are under
`evidence/native-retained-owner-delivery-20261022/`.

CI deliberately skipped per the user's instruction. No QEMU/browser/image,
protected-service execution or release acceptance is claimed. #6201/#5994 remain
open for actual Root custody, admitted caller paths and original acceptance.
