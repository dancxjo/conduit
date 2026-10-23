# Retire the browser resident Patchbay legacy Fore

Refs #6201 and #5994; delivery PR #6255.

The current installed `plots/patchbay/main.conduit` uses the checked
FaceInteraction Fore. The final caller audit found a remaining product route
from `application/retained(application="patchbay")` to the old event/view
adapter when no canonical Mask was selected. Its named Source already lives
under `plots/patchbay/fixtures/legacy-retained.conduit`, but the adapter was
still constructible in a product build.

Product preparation now requires the canonical Kind before actor allocation
and verifies that the owner placement belongs to the exact Body Plan. It
prepares the same single retained actor and TextInspection delivery; no second
graph, handler or reducer is introduced. PreparedPatchbay defaults to canonical
Fore behavior. Empty polls and legacy ApplicationEvents cannot emit an
ApplicationView, advance its actor or retain an owner request. Typed current
Face/Show interactions still use the existing publication/ingress authority.

The old event/view execution, busy-view producer, input mode and legacy
constructor are `cfg(test)` migration fixtures. Named old Source tests keep
their historical meaning; product builds cannot select that constructor.
This is a fence, not a permanent compatibility reader. The portable library's
view vocabulary and Tour's separate supported Canvas consumer remain intact.
The native actor's existing canonical-slot event fence and unchanged legacy
3 KiB admission are preserved.

The two production helper files are disjoint from #6252's changed files. Its
PreparedApplication boxing and whole-Body SDK Mask repair remain owned there;
this work does not modify or absorb that branch.

Validation:

- Three focused tests pass: retired Fore refuses before Source/actor preparation;
  canonical product emits no empty/event view and leaves revision/request state
  unchanged; forged owner placement cannot escape Body Plan custody.
- All 31 browser-runtime Patchbay tests pass, including current canonical
  publication/command admission and named legacy CPU migration cases.
- Workspace formatting and whitespace checks pass.

The non-test WASM product check passes (`wasm32-unknown-unknown`, library
target), establishing that the product cfg compiles without the legacy fixture
methods. This is compilation, not browser enactment. Earlier production Chromium/QEMU packets retain their original
Sources and are not relabelled as post-fence enactment. Final combined-head
Candidate/Integration, browser replay and normal merge are still required.
No parent issue, larger-graph or provider/fallback acceptance is closed here.

Strict browser all-target Clippy was run and remains failed on two baseline
errors: PreparedApplication's unboxed large Patchbay variant and an obsolete
`subject_identities().into_iter()` in publication tests. Both exact fixes are
already present on #6252's `e80ddbbe78dec031ed2949177aa9c8a55395a42e`;
no diagnostic was reported in the two changed helper files. They are not
duplicated or suppressed here, and strict combined-head lint is not claimed.

[Retained logs and tested helper hashes](evidence/patchbay-legacy-fence-20261022/)
record the 31-test suite, production WASM check and known failed lint.
