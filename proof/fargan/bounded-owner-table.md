# Finite runtime binding arrays

`conduit_core::bounded_owner_table::BoundedOwnerTable<K, V>` owns one sorted
`Vec<(K, V)>` with an explicit maximum entry count. `storage_reservation` checks
multiplication and the Rust allocation-size boundary without allocation.
`with_storage_limits` refuses either insufficient preparation or retained-array
ceiling before allocation. Insertions, lookups, removals, and reuse never grow
that array. Duplicate/full insertion returns the exact original key and value.

The receipt charges the full reserved array, including unused slots. It excludes
nested key/value allocations, shared-owner headers, the inline root, allocator
bookkeeping, and stack. Those remain separate owner inventory obligations.
Entry capacity is a binding-table resource bound, not a model/window capacity.

The actual trained FARGAN fixture uses these arrays for its expression and filter
Host owners. It preserves original node keys and exact prepared owners, Source,
Plan, resource custody, scheduler topology, and transport limits. It drops
preparation-only encoded inputs, fixture reference maps, and filter factories
after transferring their material to runtime drivers/storage. The existing
numeric factory/adopted maps already dropped before execution.

Allocator tests cover exact requested array storage, both one-under refusal
ceilings with no allocation, original Arc identity on full/duplicate refusal,
borrowed String lookup, sorted insertion/reuse, zero capacity, and overflow.
This is a component ownership improvement; complete concrete public driver
working-memory/preparation admission remains separate.

The target-specific adapter migration must use this lower-layer owner after a
coherent dependency rebuild; existing immutable SDK artifacts are unchanged.
