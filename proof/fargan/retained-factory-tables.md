# Bounded retained numeric profile selections

The Guard, structural Pair, Flow-pair, float-to-integer, nominal Weakening and
U16-profile factories move their original transient BTreeMap selections into
Core `BoundedOwnerTable` before returning. The exact number of selected entries
is reserved before allocating the sorted array; original keys/offers/profile
owners are moved, not reconstructed or normalized. Lookup and the unchanged
complete placement checks remain in their original owner. The bounded array
never grows at runtime. Original duplicate rejection during preparation remains.

Local payload inventories count actual array capacity, identity/key String
capacities, full retained offers and owned structural profile payloads. U16 and
Weakening shared profile Arcs are visited by borrowed identity without allocating;
the aggregate must charge each shared allocation and its original full profile
payload once. The profile payload methods include complete Types, strings,
contract vectors/paths and all U16 invariant programs. No laws are omitted.
Inline factory roots, shared Arc roots/headers and allocator bookkeeping are
separate charges. None/overflow is refusal, not a zero-byte inventory.

The Flow-pair factory additionally supports independently prepared immutable
profiles. It still verifies the complete Plan and complete original offers;
missing or ambiguous profile selection refuses. This avoids regenerating the
same Source catalogs for every placement. It does not replace the original
legacy constructor or admit a new profile implicitly.

Allocator gates compare profile inventories with actual retained capacities,
including a nonempty U16 invariant. The actual archived 958-placement Plan
compares the bounded retained Flow-pair inventory with live requested payload,
and checks all selected budgets against the legacy constructor. The new profile
reuse path retains the same per-placement budgets; cloning original offers may
trim spare capacities, and its inventory separately reports those actual capacities.

This checkpoint removes only these six factories' retained BTreeMaps. Tensor
factory maps, complete transient factory/catalog/checker preparation peaks,
complete public driver preparation and trained execution remain separate work.
Moving maps is not upstream admission of their transient construction. #5218's
proper IPA, overlapping gestures, same original projectors, frontier/timing and
attended contrasts remain required; no pitch/PCM component substitutes for them.
