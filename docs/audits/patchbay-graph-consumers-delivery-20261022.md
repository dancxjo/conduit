# Shared Patchbay graph meaning in the existing consumers

This #6201/#5518 slice reuses native-locala79ea9888's two graph-consumer changes
on top of #6235. Native FrontDoor now delegates graph facts directly to the
shared projector; the browser workbench delegates Gear, checked composition,
Port and Cord facts while retaining its labels, icons, controls and exact
Plan/Play/Line augmentation. The native duplicate projection is removed.

Browser workbench projection now includes the declared root Fore Ports, which
were previously omitted. Their Cord endpoint relationships name actual Port
subjects. Missing endpoints retain their exact source/sink properties without
fabricating subjects or connectivity. Temporal wording uses the checked
contract vocabulary, and recursive/declaration provenance is preserved.

Existing production input/effect routes and resident Source are unchanged.
This is a graph-meaning consumer migration, not completion of the resident
FaceInteraction Fore, Host acknowledgement, private storage admission or editor
handoff. No Look, geometry, global capacity, dependency or scheduler changes.
The existing browser projection file shrinks; its Plan/Play augmentation stays
beside the same callers rather than being discarded for a smaller common model.

Nine focused hosted cases pass: seven existing workbench projection cases retain
Body/Plan/Play/Sign and source-scope facts; one native graph-to-Face case compiles
the native-compositor profile and prepares its actual scene; one boundary case
compares browser graph identities/properties/topology against the complete shared
projection used by native, including root Fore ports and a missing-endpoint
fixture. The malformed fixture tests readable missing facts, not graph admission.
The initial new test failed to compile due to reversed helper arguments; that
negative is retained separately from the corrected passing comparison.

The final workbench all-target check and changed-Rust formatting pass. Logs and
checksums are retained in `evidence/patchbay-graph-consumers-delivery-20261022/`.
These are CPU projection/compilation results, not browser input, QEMU, physical
or actual owner acknowledgement. CI is deliberately skipped per the user's
instruction. #6201/#5518/#5994 and their remaining original acceptance stay open.
