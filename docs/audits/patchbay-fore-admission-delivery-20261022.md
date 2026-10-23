# Native Patchbay Fore admission — 22 October 2026

Refs #6201/#5994. Preparation now has an explicit typed Patchbay interaction offer and an exact authored-Fore planning/admission seam. It reuses ordinary authoring, planner and fragment-set lowering. The accepted profile has one bounded input, one owner Host Call and an exact memory reservation, with no extra authority or alternate actor/scheduler.

Admission independently lowers the Plan and compares contracts, resources, Host/Boot/offer identity, Fore and node/Cord/Host Call structure after normalizing only the actual aggregate indices and admitted queue slot start. Unexpected state, fusions, remote endpoints, resources, routes, authority and changed sinks refuse. Installed Source, Host advertisement and callers are not switched in this preparation-only slice.

One focused hosted test passes: check/expand a canonical Source fixture, prepare the existing native workset, plan against an explicit augmented Host offer, combine ordinary fragments, verify actual nonzero node/Cord indices, and refuse a foreign sink. The fixture is deliberately separate from the still-legacy installed Source. Freestanding x86_64 native-compositor library compilation and formatting pass. An initial test-helper name shadowing compile failure is retained separately; it was corrected before the final passing run.

CI deliberately skipped tonight. No protected guest execution, real Mask acknowledgement, browser/QEMU enactment, native stack/private-heap fit or full #6201/#5994 acceptance claim. Logs and hashes are retained beside this audit.
