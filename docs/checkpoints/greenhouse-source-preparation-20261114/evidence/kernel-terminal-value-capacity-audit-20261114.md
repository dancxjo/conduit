# Terminal-value capacity audit

Bounded read-only review; no source change or build. Exact generated Source76 layout SHA256 `a87386b13ce7e42ab405ed8c6d9d03b938dc60f8667be99543772e86498a4879`: the complete generated terminal matrix contains18 present contracts, all with abnormal `NotAccepted`, and **zero `FinalizeThenPropagate` contracts**.

## Immediately justified narrower opportunity: abnormal phase payloads

`ActiveTerminalPhase::Abnormal` embeds a100-byte-capacity CanonicalValue in the enum used for every node's retained phase. Its only NEW construction is `scheduler.rs::validate_terminal_transduction` (~2634), specifically the `AssignedAbnormalTransduction::FinalizeThenPropagate` + `StepOutcome::Progress` branch. `advance_terminal_phase` only retains that existing phase under the same contract.

Therefore separate compact ordinary phase progress (input, emitted counts, phase tag) from the optional abnormal phase's retained bytes. Fund separately indexed abnormal-phase payload stock from the exact actual prepared terminal contracts. The constructor must refuse a stock/profile that cannot cover every node with `FinalizeThenPropagate`; the original actual-Back/linked-table checks remain. The current Source76 matrix justifies **zero abnormal-phase payload slots**, while retaining normal-close phase behavior and all ordinary abnormal outcomes. This is an exact selected-profile specialization, not a smaller global CanonicalValue limit or an assumption about sensor behavior. A default fully general profile can still fund all original possibilities. Parent's actual ARM geometry should determine the savings; no ABI-size estimate is asserted here.

## Not justified: deleting Cord or unresolved abnormal payloads

`NotAccepted` describes handling an incoming abnormal value; it does **not** prove that a Back cannot spontaneously return `StepOutcome::Abnormal`. If no input terminal phase is selected, `validate_terminal_transduction` may return no phase, and `apply_step` still processes that emitted outcome. `abnormally_terminate_outputs` retains abnormal truth on sibling lifecycle Cords and in `unresolved_abnormal`; remote ingress can also retain an externally supplied CanonicalValue in CordState. These meanings must remain even if the normal Source protocol reports failures as ordinary variant payloads.

There is a rich lowering contract: `LoweredPort.maximum_abnormal_value_bytes`, populated from checked InputAbnormal/OutputAbnormal contracts in `architecture/plan-lowering/src/lowering/ports.rs`. Explicit abnormal-track Cord bounds use it (`lowering.rs:982`). However, `make/embedded-build/src/model.rs::GeneratedPort` retains only node/port IDs and payload Kind, and assigned Port records likewise lack an explicit per-port abnormal envelope. Fore ABI digests pin richer contracts, but the kernel is not currently given decoded, independently checked per-port abnormal extents for these retained state arrays. No present proof justifies shrinking their100-byte bound wholesale.

A future shared bounded terminal-value owner could retain one payload with handles across Cord/phase/unresolved fanout, but it needs truthful simultaneous-live admission and reference/retirement semantics. Simply borrowing the ordinary value pool without funding that stock is unsafe accounting. Do not start that larger redesign to achieve the phase-only reduction.

## Other strong candidate

After linked terminal contracts, `node_specs`, `cord_specs`, and sealed FixedRoutes are immutable throughout Play and reconstructible from checked assigned topology. A linked topology storage profile is the strongest other existing metadata direction. It must retain local-Fore provenance and boot-specific exact assignment checks. Mutable Cord queues, pending calls, terminal progress and unresolved abnormal truth remain RAM.

Tests for a phase-storage change must preserve unexpected spontaneous abnormal outcomes, remote abnormal closures, ordinary variant failures, normal flush closure, actual `FinalizeThenPropagate` behavior under the general profile, and early refusal when its specialized phase stock is insufficient. Keep original grants and MAXIMUM_DERIVED_VALUE_BYTES unchanged.
