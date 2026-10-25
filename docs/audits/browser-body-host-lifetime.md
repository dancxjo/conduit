# Browser BodyHost lifetime fixture

The focused Chromium entrance is `proof/browser/browser-lifetime.config.mjs`.
It runs one case with one worker and zero retries against the production browser
runtime WASM and the production browser BodyHost, runtime bridge, membership
client, Plot checking, Workspace proposal, and effect dispatcher.

The fixture admits A, retains its actual timer effect identity and real input
and timer callbacks, closes A, and admits fresh B on the same WASM instance and
bridge. It replays A's exact active Play, placement, and request sequence through
completion, cancellation acknowledgement, and refusal exports. It checks refusal,
input staging retirement, unchanged B evidence and outputs, then invokes retained
A callbacks and checks unchanged B state. A settled B baseline follows its
actual pointer-up manifestation. After invoking A callbacks, a separate browser
evaluation and MessageChannel task fence drain promise continuations before
comparing B with that original baseline; the receipt retains `afterCallbacks`.
The immediate ABI snapshot alone cannot establish delayed callback isolation. Positive controls advance B's timer and
observe its count output, with its button indicator sibling unchanged, then
exercise B's actual pointer input and require the indicator to change.

The observer wrappers delegate every call to the actual WASM exports. They only
copy actual output tuples. They do not provide scheduling state, effects, plans,
or responses. The page clock controls elapsed browser time; timer callbacks and
DOM listeners come from the actual production adapter.

This is a page-owned WASM/BodyHost lifetime fixture. It establishes no execution
WebWorker termination, Watch, physical authority, native process containment, or
full #6409 acceptance. The sibling control is a separate Plot placement, not an
independent browser Host. It does not claim that raw matching B-owned tuples are
cross-owner capabilities.

Local validation: the fresh supported WASM release build and the focused pinned
Chromium case passed with one worker and zero retries. The first actual attempt
was retained: the fixture initially inspected the input-driven sibling before
establishing its first B input. The correction establishes that actual sibling
manifestation before the no-effect comparison. Production source is unchanged.
The proof packet binds the actual loaded WASM hash to the source, lockfiles,
build environment, and pinned Chromium binary; the passing case retains a JSON
receipt and screenshot. This is local Chromium evidence, not Candidate or
Integration acceptance.

Review correction: the initial passing fixture compared synchronously after
callback invocation. That snapshot did not cover later promise continuations.
The strengthened case retains both immediate and post-settlement comparisons;
the earlier packet remains evidence of its narrower assertions only.

Current-dev validation: after merging dev `d3356f28a` into this focused proof,
source `ad37ef411ebe3b06a3e2c078675b142ee7c761e4` built the production release
WASM with the repository browser settings and passed the same pinned Chromium
case with one worker and zero retries. The fixture confirms the loaded WASM
digest `5108d6c53b35f1a739f3d91c01321e4ecb998a59858f7056e3ab8a0b5019aaf8`;
the exact tested bytes, receipt and logs are retained outside Git. The case is
registered in the regular Playwright configuration used by the existing browser
proof entrance. This remains local execution, not Candidate or Integration.
