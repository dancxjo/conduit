# Fanout stock opportunity: bounded measurement, not an admission proof

Read-only analysis of the emitted Source76 projection and original value profiles. No build, production edit, grant or queue change. Reproduction: `scripts/audit-source-fanout-20261114.py`; exact artifact hashes and all groups are in `source-fanout-opportunity-20261114.json`.

The selected graph has95 Cords/97 queue slots and81 schema-backed queue slots. Ten groups satisfy all of these descriptive guards: same local source output, every sibling included, identical schema/max profile, local sinks, Payload track and PreserveOrder pressure. They account for16 queue reservations beyond the maximum sibling queue capacity. Their per-group node-payload estimate is6616 bytes.

The prospective argument is that atomic fanout of one ValueRef and FIFO head consumption make queued values overlapping suffixes of one producer stream, rather than independent payload objects. That could permit physical stock based on maximum sibling queue occupancy while preserving every original queue, logical slot, reference count and generation. It must not be applied to CoalesceLatest, mixed profiles, external routes, incomplete sibling inventories or unknown lifetimes.

This measurement is NOT a runtime saving or a complete stock calculation. The current constructor also uses class-wide maximum extents, transient outputs, retained state and native pins. Those terms must still be funded. A different class packing choice could change the net payload/metadata saving.

Before implementation, prove the suffix/lifetime invariant through pressure, atomic commit/rollback, failure after partial work, cancellation, and reusable-session rearming. Public scheduler error/retry behavior must not be ignored merely because the intended product runner usually stops after an error. Preserve the dense path on an unproved case and test old/new reference and Sign traces. Then repeat the actual prepared-owner whole-factory and target measurements.

This is a bounded candidate under existing workspace custody, not a new transport or MCU epic. Do not count6616 bytes as delivered savings or claim it makes the appliance fit. The latest successful actual CPU preparation remains159165 retained/161204 peak; complete target fit and actual local admitted sensing/display remain unfinished.
