# Exact Cord source provenance prerequisite

This focused #6201 prerequisite reuses the reviewed Plot preparation delta from
native-local0697bad9a, principally50fb431d1,ecbccc0ae anda99146926. It does not
switch resident Patchbay Source, add an owner/Mask route or deliver the unrelated
selected-Look/native recovery branch.

Checked Cords retain their complete lexical span without changing semantic Cord
equality. Expansion carries origin alongside each exact connection: ordered
endpoints, track, value Kind, temporal contract, declaring document/Plot, instance
path and declaration span. A selected Back binds its own document before recursive
inventories merge; endpoint spelling does not infer lexical ownership. Root Fore
bindings retain their declaration origin separately from internal connections.

The provenance digest now includes connection origins; root bindings have a
separate digest. Validation rejects stale mappings, track/endpoint substitution,
removed rows and changed declaration coordinates. These digests are preparation
evidence, not edit authority, Host realization, storage grants or confinement.
Old retained receipts keep their own digest representation and source identity.
Semantic graph identity remains separate from location/provenance identity.

The change extracts Cord checking, connection materialization and provenance
encoding into responsibility modules rather than growing the existing large
checker/expander files. New modules stay below500 lines. The only outside callers
changed here are three explicit test-fixture initializers that now supply the
new origin/span fields; no other package's product code is absorbed.

Twenty focused local checks pass: three connection-origin cases, one checked
Cord span/equality case, nine existing first-class-unit/source-evidence cases
and seven semantic-call lowering cases. The final owning Plot package all-target
check and changed-Rust formatting pass. Logs are retained under
`evidence/patchbay-cord-provenance-delivery-20261022/`.

No CI, browser, QEMU, product-image or stable-release proof is claimed. This is
only the prerequisite that lets the following Patchbay graph/Face/typed-owner
slice preserve exact Source facts; #6201 and parent #5994 remain open.
