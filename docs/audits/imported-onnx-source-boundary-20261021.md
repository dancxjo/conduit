# Imported ONNX Source selection boundary

Issue: #5219. Actual hosted ORT execution remains separate work in #5220 and
#5694; this change does not load ORT or download models.

The previous model-inference suite passed 47 tests across 17 targets, but its
checked Source/Plan/Play fixtures prepared reference-linear resources. Imported
ONNX graph metadata and checkpoint-local name correspondence were tested
separately. Those results did not establish custody across the planning seam.

`PreparedModelInfer::prepare_imported` now retains the checked imported owner.
Its capability and resource selection identity binds the admitted descriptor
and sidecar identity. The existing native preparation identity stays unchanged.
Borrowed invocation views check shared ONNX symbols across inputs and outputs
before provider work and before accepting its reply. The fixed 256-entry symbol
table covers the admitted maximum of 32 total ports with eight dimensions each.

The exact Add/Sub test fixture checks two semantic inputs and outputs through
ordinary checked Source, planning and the production kernel. Its fixture Back
recognizes only the pinned graph bytes and explicit name maps. Graph bytes or
valid name-map changes alter the selected capability and Plan while preserving
the authored Source identity. Printed evidence includes graph SHA-256,
signature, sidecar, name maps, Source, Plan, Play, Host and Boot identities.

The second exact fixture is ONNX Identity with a shared `sequence` symbol,
bounded to widths one through two. The same checked plan executes widths one,
two, then one and checks the exact output bundle and unchanged kernel value
allocation capacity. Width three must refuse with typed InvalidInput and no
output. The reference echo Back is guarded to the exact Identity graph and
checkpoint-local name map.

This is portable fixture evidence. Metadata inspection does not certify an
arbitrary graph's operator semantics; each selected Back remains responsible
for its admitted execution profile. No native allocation, physical device,
hosted ORT, release, or live-publication claim follows from these tests.

Validation at `1202b49c01baea35017d93d578ca2b8717d4f775`:

- Supported `cargo +stable xtask check model-inference`: 50 passed across 19 targets, zero failed or ignored.
- Exact borrowed-symbol unit: one passed (880 unrelated tests filtered).
- Workspace formatting check passed.
- Strict Clippy stopped on existing, unchanged core execution and std kernel-preparation findings. Package-scoped negative diagnostics are retained; no lint-clean claim.
- First dynamic test compilation consumed its expected bundle; the retained negative log precedes the corrected consuming-encode test and passing suite.

Raw logs and hashes are in `evidence/imported-onnx-source-boundary-20261021/`. This remains unmerged preparation, not issue closure or accepted release evidence.

## Exact Back admission and selected-Plan inspection follow-up

Validated executable source: `996cefe6e85c2991f51a773cfcad5b2494d21bd4`.
The imported preparation entrance now requires an exact checkpoint Back profile,
including runtime and artifact identity, execution profile, IR/opset bounds and
operator vocabulary. Unsupported profiles and mismatched custody refuse with
typed errors before the provider load call. This is exact fixture admission;
operator names alone never certify arbitrary graph semantics.

The actual selected Plan now carries a bounded native inspection property with
checkpoint identities and semantic-to-graph input/output correspondence. Reading
it verifies the selected implementation, artifact, execution and Back profile.
The imported selection identity also binds admission. Provider replies must
retain the admitted runtime identity. Existing native preparation stays separate.

Supported `cargo +stable xtask check model-inference` at that source passed
**57 tests across 21 targets**, with zero failures or ignored tests. This includes
six AI admission/inspection tests and the std pre-load refusal matrix. This
supersedes the getter-only inspection and fixture-only admission limitation of
the earlier checkpoint. The previous strict-lint failure remains negative
evidence; no new lint-clean or integration/publication claim follows.

## Current-dev recovery for #6219

The imported custody/admission slice is tracked separately as #6219, prerequisite
for scalar tensors in #6210. On current-dev product source
`c08880393` the supported model-inference suite passed **58 tests across 21
targets**, zero failures or ignored tests. The focused borrowed-symbol unit also
passed. A new ordinary Source/Plan/Play regression checks that a provider reply
from an unadmitted runtime fails after one load and one invocation, emitting no
output. Source-bound logs and hashes are retained in
`evidence/imported-onnx-current-dev-20261021/`.

These local hosted fixture receipts do not establish Candidate, Integration,
merge, actual ORT execution, or parent #5219 completion. Historical positive and
negative receipts above retain their original source identities.
