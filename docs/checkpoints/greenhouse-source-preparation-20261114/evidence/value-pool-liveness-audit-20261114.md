# Source76 value-pool lifetime audit

Read-only audit; no build, production source edit, deployment, or measured fit claim. The generated layout is the actual Source76 candidate at `work/cache-backing/rp2040-source-fit-20261113/thumbv6m-none-eabi/release/build/conduit-rp2040-host-c79668900a21695b/out/workset_0_layout.rs`. Adjacent JSON records exact layout hash and all slot extents.

## Finding

`targets/mcu/source-build/src/template.rs:205` and `value_profiles.rs:176` reserve staging for every routed output in the whole graph. `native_values::prepare_schema` allocates all Cord and all Step profiles together. The actual76-node projection has76 outputs, exactly ONE per node. `scheduler::apply_step` runs materialize→commit/rollback→step_committed synchronously before another node runs. Thus staging stocks for all76 nodes cannot be simultaneously needed as staging.

Current CPU64 pool114,426B decomposes into56,682B Cord payload +44,080B Step payload +4,608B native results +9,056B slot metadata. Cord stock97slots includes81 schema slots and16canonical; Step stock76slots includes63schema and13canonical. All28 exact schema groups have uniform node capacity within the group.

## Recommended smallest safe first change

Keep every original Cord physical slot, every canonical Step slot, native result slot and logical quota. Consolidate only schema Step stock to the maximum simultaneous outputs of ONE node for EACH EXACT SCHEMA (one per schema for this graph). Add explicit retained-owner stock separately. This removes35schema staging slots:20,493B payload +1,960B CPU64metadata =22,453B. Two actual prepared SeededUntil owners (nodes8,28) retain one input each outside Cord occupancy: schemas5/13 need387/709B node payload +112B metadata. Conservative candidate reduction21,245B, pool114,426→93,181B CPU64 requested backing. This is a derived candidate, NOT implementation or measurement.

A more ambitious canonical collapse could yield another7,718B, but do not include it initially: varying canonical slot sizes bring best-fit fragmentation obligations, and consumed Host Call inputs stay pinned while queues refill.

## Ownership contract

No existing StepBack static retained-input declaration exists. Add an explicit opt-in physical retention description on the ACTUAL PREPARED Back, analogous to workspace/schema declarations. Default must be Unknown (retain original stock or refuse compact profile), never assume zero. Prepared stateless selectors/residents/merge/Zip declare no retained ValueRef; SeededState declares at most one retained input across its seed/replacement alternatives. Driver and SoftwareBack delegate to exact selected owners. Declaration refers to input Port/schema and simultaneous count, not semantic Kind strings or BME node IDs. Match independently emitted physical facts against actual prepared owners before choosing compact stock; actual backing remains charged by Root.

Native pending inputs are separately derivable from existing admitted HostCallBinding.maximum_input_bytes and pending-call bounds K/H. Current two HostCallBacks consume and pin canonical requests407/156B, one each; leave their original canonical staging stock intact in first change. Existing native results remain4,096+512B. No guessed254-slot stock is necessary: construct exact per-schema Cord occupancy +per-schema single-Step maxima +declared held references; canonical side remains original.

## Lifetime caveats and required proofs

- A materialized Step output becomes the queue-owned ValueRef after commit; it is NOT a temporary byte buffer that can be overwritten next Step. Compact stock is shared free SLOT capacity, not one mutable staging buffer.
- SeededState::step uses take_input and can hold a consumed input after its queue ref disappears. During replacement, old held state +queued replacement +new output may coexist. One extra held slot per state owner covers the former; original Cord slots and one-Step stock cover latter two.
- Kernel HostCallBack consumes input, and PendingHostCall owns it until completion; output result may coexist. Do not count only queue occupancy.
- Remote egress remains in its Cord queue until remote_egress_delivered pops/releases it. Existing Cord slots already cover this retention; an offered sequence does not allocate another ValueRef. Coalescing/acceptance/rollback must retain existing behavior.
- Ordinary Sign events carry identities, not owned ValueRefs; debug observers receive borrowed byte slices. Unique durable/evidence retainers must declare their own custody rather than assume all Signs are free.
- Value generation is u16 and slots permanently retire at maximum. Fewer slots can shorten aggregate generation lifetime. Require an admitted finite Play horizon that cannot exhaust the smaller stock (or preserve original horizon/fallback); do not silently recycle stale references or claim arbitrary continuing operation.
- Existing logical Cord maxima, queues, fanout, scheduling order and original Source identity remain unchanged. New physical profile is explicit, independently checked, and opt-in. Preserve original semantic byte allowances separately from compact backing ceilings; SchemaValueStore currently bounds byte_capacity against installed logical slot sum, so do not silently shrink semantic budgets to satisfy its constructor.
- Test queue-full fanout, held state with queued replacement, concurrent pending native requests/results, remote offered/accepted/coalesced values, multi-output nodes, rollback/sign-capacity failure, generation retirement and zero-allocation execution. Then perform actual complete signed-command preparation and ARM heap/stack qualification.

Code evidence: native_values.rs:92; schema_value_store.rs:160 (best-fit exact-schema slot selection); scheduler.rs:2293 (serialized materialization/commit), :2856 and :2990 (retained-input/reference accounting); scheduler/payload_admission.rs:125,:253 (workspace copied into owned value store); composite/seeded_state.rs:330; scheduler/host_call_back.rs:61; scheduler.rs:1636 (remote queue release).
