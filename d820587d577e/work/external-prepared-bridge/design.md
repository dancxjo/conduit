# Exact owning-crate prepared Native descriptor bridge (draft)

Existing generated defaults, complete laws, nominal IDs, runtime64/root16, generation256 remain unchanged.

Owning Data generation first emits complete prepared TensorElement/TensorAxisRole descriptors/converters using its checked original Source. Do not implement external traits in importing AI.

Add separate ExternalPreparedNativeRustBinding input, keeping ExternalNativeRustBinding unchanged. It records semantic identity, Rust path and exact owning descriptor metadata. The code generator validates full canonical imported Type against descriptor.type_bytes and walks every external child (bounded, cycle-safe); these count toward each root's complete64 ceiling and shared generation256 inventory.

Emit imported child references as `<owning_path as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR`. The importer must also embed an expected external edge contract containing full expected canonical Type, ordered law bytes and all leaf-contract recipes. This closes mismatch between the reviewed generation input metadata and the actual Rust path used by generated code.

NativeFamilyTypeDescriptor gains external_edges: &'static [NativeFamilyExternalEdge] (empty for existing local descriptors), where an edge contains actual original owning descriptor plus exact static expected Type/laws/contracts. PreparedNativeFamily::prepare validates these bytes allocation-free before any owned allocation, materialization or target consumption, and refuses missing/foreign Type, omitted/reordered/changed laws or changed constraints. Existing child pointer membership and complete recursive validation remain. Static expected metadata is immutable program storage, reported separately from retained owned heap; its encoded/structural extent has finite generation bounds. Do not count encoded bytes as temporary allocation ceilings.

Runtime collection must additionally compare each reachable named child schema's exact full Type against its referenced descriptor (including local edges); no canonical-byte equality is substituted for generic value equality. The complete contracts/laws then undergo existing bounded preparation, fresh full canonical input validation, generated recursive conversion and all child laws.

Meaningful gates: real Data TensorElement/AxisRole ALL variants; imported ModelSignature closure; wrong Rust-path descriptor with equal nominal ID/different Type; exact Type but missing/reordered law/changed contract refusal before allocation/target; foreign pointer absent; complete external64/65 and generation256/257 boundaries; full default Reference output/refusal parity; no_std owning/importing crates. No Source guard drops, categorical-only trimming, orphan impls or global caches.

This is a design only; no generated external admission claim.
