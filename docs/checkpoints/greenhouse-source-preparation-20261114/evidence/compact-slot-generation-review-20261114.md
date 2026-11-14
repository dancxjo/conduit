# Compact physical buffers with original logical generations

**Accept the design, conditionally.** Keeping the complete original logical slot domain removes the generation-lifetime objection. This is an explicit prepared physical storage profile, not a claim that fewer logical items or smaller authored quotas suffice. No builds or source edits were performed for this review.

## Reference domain and public capacity

`ValueRef { slot:u16, generation:u16, byte_len:u32 }` names a logical slot. Preserve all original schema logical slots in their original order, each original exact schema, individual maximum, generation and reference count. Preserve canonical store item count and therefore the schema offset (`canonical_slots`) exactly. `ValueStorage::item_capacity` remains the original total logical reference-domain upper bound; `byte_capacity` remains the original logical allowance. `used_items/used_bytes` count currently live logical values/canonical extents. They must not count borrowed schema bytes as physical RAM.

The current trait does not promise that every arbitrary set of `item_capacity` values can be stored: its existing heterogeneous schemas and extents already refuse unsuitable values before that global count. Thus unchanged item_capacity is truthful as a logical upper bound. Nevertheless, **do not use that number alone to certify physical suitability**. The compact profile must expose/check its actual per-schema simultaneous-buffer bound and actual backing. Calling it a fully general dense replacement without a checked lifetime inventory would misrepresent its capability.

Leave `SchemaValueStore::try_new` dense and unchanged. Add an explicit compact construction path that accepts the complete original logical definitions, original logical byte allowance, separately checked physical stock, and an opaque checked lifetime inventory for the selected graph/actual prepared owners. Unknown ownership declarations select dense physical stock or refuse compact preparation. Do not let an arbitrary caller pass a small physical count and claim the original admission merely because logical item_capacity matches.

## Minimal owner split

A minimal implementation can stay within the existing schema-separated store and ValueStorage interface:

- Immutable schema/class table borrows each exact schema and validator once (maximum validator ceiling must dominate every class member; per-logical-slot maximum remains separately enforced).
- Logical slot table contains class identity, original maximum/selection extent, generation, references and optional physical-buffer index. Retain original order and choose the same first/best-fit logical slot as today, using immutable logical extents rather than the currently assigned physical buffer capacity.
- Physical buffer table contains preallocated node Vecs, explicit live/free ownership and optionally owner logical index for invariant checks. It carries NO externally visible generation. Zero-length nodes are valid, so node length cannot encode free status.
- Store first performs existing byte-quota and logical-slot checks, validates the complete node and exact schema/individual maximum, selects a free matching physical buffer, and proves both assignments. Only then increment logical generation, attach buffer, copy node, publish references and update counters. A shortage must not consume generation or mutate either owner. Expose a distinct physical-capacity refusal/diagnostic rather than implying logical item count was exhausted.
- `get/canonical_parts/reference_count` validate logical index+generation+live refs, attached buffer ownership/schema and full canonical length. No reference points directly to a physical buffer index.
- `retain` changes logical refs only. `release` on the LAST ref detaches/clears the physical node and returns its buffer to stock; the logical generation remains. `clear` detaches every live buffer, resets live counts and PRESERVES generations as it does now.
- A logical slot reaching u16::MAX permanently retires exactly as before. Its last release still returns its buffer; another original logical slot can acquire that SAME physical buffer under its own generation. Do not retire physical buffers with logical slots. No generation widening or stale-reference recycling is needed.

The mapping costs metadata. The earlier21,245B candidate estimate removed logical slot metadata and therefore does NOT transfer unchanged to this design. Schema-only payload saving is independently19,397B (20,493B removed repeated Step payload minus1,096B added held-state payload). Actual metadata and full preparation peak must be measured after implementation. Grouping immutable validators once may offset mapping metadata, but this review makes no ABI-size saving claim.

## Physical lifetime inventory

For each exact schema, keep all original Cord queue stock, add the maximum simultaneous materialized outputs of ONE Step, and add independently bounded driver-held inputs. Source76 has one output on each of76 nodes;63 schema Step slots cover28 exact schemas. SeededUntil nodes8/28 each hold one input (physical node387/709B). At replacement, previous held state, queued replacement and newly materialized output coexist; the three stock terms cover these separately.

The contract must describe the actual prepared Back, not the Kind name, BME node number or Source text. Add an opt-in static retention declaration analogous to actual prepared workspace/schema support; default Unknown. Delegate through Driver/SoftwareBack and bind it to the independently selected implementation/artifact. Describe alternatives as at-most-one across seed/replacement inputs, not one simultaneously for each input.

Keep every canonical Step slot and native result slot for the first implementation. Pending Host Call input ownership is derivable separately from admitted HostCallBinding.maximum_input_bytes and pending request bounds; original current native requests407/156B remain canonical. Native results4,096/512B remain separate and may coexist with requests. A larger canonical pool reduction needs a separate variable-size fragmentation proof.

Remote offered/accepted values stay in their original Cord queue until delivery pops/releases them; queue stock already covers those refs. Ordinary Signs retain event identity and debug values are borrowed, not extra value-store owners. Any installed observer, foreign bridge or durable path that retains a ValueRef must independently declare its bound or force dense fallback. External/canonical ingress stays on existing canonical storage; do not infer schema capacity from a remote Kind match.

The factory must verify the physical inventory against the actual selected graph and prepared owners before Play, preserving exact Source, Cord queue counts, byte maxima, output fanout, scheduling order and original logical quotas. This inventory proves physical sufficiency over admitted scheduler traces; the storage object alone does not grant admission. Generation exhaustion can still refuse as before, but physical buffer retirement must never shorten it.

## Required qualification

1. Dense/compact differential trace: identical ValueRefs, canonical bytes, logical counters, errors and generation retirement for every admitted trace.
2. Many original logical slots with one physical buffer: repeatedly allocate/release through u16::MAX; first logical slot retires, next original slot reuses the physical buffer; all stale refs remain rejected. No build-time generation widening.
3. Shared fanout refs keep the physical buffer until LAST release; preserve immutable data while another slot publishes; clear never resets generations.
4. Whole-node multi-output materialization, partial failure rollback, sign-capacity failure and retry must detach each staged value exactly once and preserve consumed inputs.
5. Seeded state replacement with queues refilled and previous state held; concurrent native pending input/results; remote offer/accept/coalesce/delivery and cancellation.
6. Undeclared owner/foreign schema/insufficient physical inventory refuse preparation; unexpected runtime physical shortage is observable and mutation-atomic, not disguised as grant growth.
7. Exact original131072B target heap, actual original factory, full ARM32 ownership/stack accounting, zero allocation during Play. Current CPU64 original-factory evidence210354B retained/212393B peak demonstrates that this is worthwhile but does not prove this reduction alone fits.

Code evidence: `architecture/kernel/src/lib.rs:97,431`; `architecture/plan-lowering/src/schema_value_store.rs:20,160,222`; `architecture/kernel/src/scheduler.rs:2293,2856,2990`; `architecture/kernel/src/scheduler/payload_admission.rs:125,253`; `architecture/composite/src/seeded_state.rs:330`; `architecture/kernel/src/scheduler/host_call_back.rs:61`.
