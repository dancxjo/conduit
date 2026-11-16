# Source76 canonical-boundary memory audit — 2026-11-15

**Substantial candidate: preserve schema/node values up to the typed Fore boundary, then materialize the existing canonical wire bytes once into the boundary's separately funded buffer.** The largest remaining canonical stock is external output queue+Step storage, not native result slots. This is an implementation seam, not a proved saving or permission to change profiles now. Existing runtime adapters still require contiguous canonical bytes.

Bounded read-only audit; no Cargo, ARM build, stock/profile change, product edit or device I/O. Generated artifact: `work/cache-backing/rp2040-source-fit-20261113/thumbv6m-none-eabi/release/build/conduit-rp2040-host-1c5d21540636c3c8/out/workset_0_layout.rs`, SHA256 `6ba7f22b6a87397af357f93f6e93e87b966108114de28cdb6d4793132fdbb65d`. Numbers below were parsed from its actual profiles/output/sink arrays and companion `workset_0.rs`, not inferred from Kind names.

## Exact30449-byte reconciliation

| Canonical stock domain | Slots / capacity | Bytes |
| --- | --- | ---: |
| Nine external-output classes, queue plus Step each |18 |16978 |
| Three logical external ingress queues |3 |5423 |
| Four local native boundary classes, queue plus Step each |8 |3440 |
| Independent native result slots |4096 +512 |4608 |
| **Total canonical reservation** |31 logical slots |**30449** |

The raw emitted totals are16 canonical Cord-value slots=15632,13 canonical Step slots=10209, plus two Host-call result slots=4608. Separate schema-node stock is the reported55524 bytes; it is not additional immutable-prefix duplication and must not be confused with full canonical capacities.

Local native-boundary classes are node1 output841 (I2C result), node3 output407 (transaction into node1), node29 output316 (clock result), node48 output156 (request into node29). Native nodes1 and29 are explicitly selected in generated `NATIVE_KINDS`; ordinary Driver dispatch does not claim their schema support. Queue and Step extents sum1720 each.

External inputs are queue slots85→node58/port0 (2248),86→node12/port0 (1594),87→node71/port0 (1581). External output queue slots88–96 have extents2248,316,1133,157,730,1425,1190,1133,157, totaling8489, with corresponding Step outputs at nodes66,63,69,75,36,62,23,15,17. The local Step and external queue each fund a full canonical value.

## Why these remain canonical

`targets/mcu/source-build/src/value_profiles.rs::select` requires exact selected artifact/port schema declarations for producer and **every** recipient. `compatible` returns None when any recipient is external, even if all local recipients and the actual producer support schema values. This deliberately makes all siblings and that producer's Step canonical. External ingress is canonical because its source has no local selected Back schema declaration.

Actual schema support exists for the selected software owners: `template_selector_profiles.rs` rechecks exact software artifact/implementation, complete configuration and checked semantic contract, then declares selector input/output schemas. Merge declarations have a similar exact implementation path. `execution_software_schema.rs` / `execution_software_step.rs` forward prepared schema hooks; generated shared-workspace resident implementations supply their own hooks. These promises are stronger than a matching Kind name.

Conversely, `execution_factory_step.rs::Driver::Native` returns no canonical_input_schema/output_schema. `HostCallBack` simply relays the actual completion ValueRef. `native_sync.rs` calls `kernel.host_value` for contiguous input, invokes its provider into its independently bounded result buffer, then `store_host_value`. `scheduler/host_values.rs` preflights canonical storage, stores with ordinary `store`, and retrieves with `get`. SharedSchemaValueStore::get intentionally returns RepresentationMismatch for schema-backed values; callers must explicitly use `canonical_parts`. These native paths are not ready for schema-backed Host parameters/results.

The existing Fore observer also calls `kernel.values().get(offer.value)` before copying bytes to its egress buffer (`targets/mcu/host-runtime/src/fore_bridge/observer.rs`). Structured ingress validates an exact independently pinned schema but its current kernel admission path stores canonical bytes. Removing the compiler's external-recipient gate alone would therefore break execution.

## Verified immutable prefix duplication

Seven external-output selector schemas were extracted from their exact generated `SelectorStatic` canonical descriptors, using the Core selector/type encoding—not matching capacities to Kind names. Node69's exact `MergeStatic.schema` was read directly:

| Exact producer node(s) | Output schema | Prefix bytes per slot | Prefix bytes reserved twice |
| --- | --- | ---: | ---: |
|15,69 |Ht16k33Failure |900 each |3600 |
|17,75 |Ht16k33Acknowledgement |130 each |520 |
|23 |I2cCorrelatedResult |882 |1764 |
|62 |BmeProtocolFailure |1192 |2384 |
|63 |MonotonicClockResult |286 |572 |
|66 |BmeProtocolState |1571 |3142 |
|**Subtotal: eight external outputs** | | |**11982** |

This is a verified lower bound on **reserved schema-prefix capacity** within the16978 external queue+Step bytes, not a measured simultaneous live allocation saving. It excludes node36's external observation-result prefix, three ingress prefixes, four native boundary prefixes and allocator/layout effects. No figure is claimed for those unextracted prefixes. Full maximum extents also include dynamic node payload and must remain the logical ABI/grant ceilings.

For example, node66's2248-byte value includes a1571-byte immutable schema prefix and at most677 node bytes. Both its Step and outgoing queue currently reserve the full2248. A schema-aware boundary could retain the linked prefix once and keep node-only internal values while reproducing the exact same canonical output bytes. It does **not** remove the need for a finite canonical transport buffer.

## Smallest actionable existing-representation seam

Start with the exact prepared structured **Fore ingress/egress adapters**, not native Host Call ABI and not fanout/lifetime stock:

1. Prepare a boundary schema promise from the independently checked Fore ABI/pin and selected initialized adapter, with the actual logical maximum unchanged. Preserve local Fore versus network Line custody; this representation promise creates no Line authority.
2. Egress resolves existing `CanonicalValueParts` when present and reconstructs schema+node in the already admitted finite boundary buffer. Canonical-backed values keep the old path. Confirm debug/event observation does not force a second full retained copy.
3. Ingress validates the original canonical bytes against the exact pinned schema and uses existing `store_schema_canonical` only when the selected receiver/physical class supports it. Preserve pressure, generations and original request/sequence/terminal behavior.
4. Only then extend value-profile selection to accept this explicit external-adapter promise, retaining the current dense canonical fallback for unknown or mixed recipients. Differential-test exact wire bytes, all output paths, pressure, expiry/cancel and malformed schemas before measuring actual whole-factory/ARM cost.

Eight external outputs already offer11982 bytes of repeated schema-prefix reservation to investigate, making this a substantial candidate without new pooling arguments. Net savings depend on existing egress buffer capacity, actual class packing and adapter preparation backing. This audit does not prove that the complete12-port Fore lifecycle/Mask integration is installed.

Keep native result4096/512 capacities independent. They cover the actual selected native provider wire/result ABI and preflight, not merely the smaller outgoing Cord type. Reducing them to841/316 or enabling schema slots there requires exact provider contracts and a separately qualified completion/input adapter; equality of a Kind name or decoded semantic output is insufficient.

This remains #6341 immutable preparation/representation custody. It is neither the unsafe fanout stock proposal nor a new MCU memory epic. CPU cache remains untouched and idle for the parent.
