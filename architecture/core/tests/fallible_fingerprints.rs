//! Fallible descriptive fingerprint output preserves canonical IDs, not authority.
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::*;
fn root() -> PlanFingerprintInputRef<'static> {
    PlanFingerprintInputRef {
        source_document_id: "來源:α",
        checked_plot_id: "",
        expanded_plot_id: "展開",
        realization_backs: &[],
        activations: &[],
        commitments: FragmentCommitmentRefs::Borrowed(&[]),
        activation_preparations: &[],
    }
}
fn header() -> FragmentFingerprintHeaderRef<'static> {
    FragmentFingerprintHeaderRef {
        source_document_id: "來源:α",
        checked_plot_id: "",
        expanded_plot_id: "展開",
        completion_policy: PlanCompletionPolicy::SemanticCompletion,
        realization_backs: &[],
        host_id: "host/環境",
        boot_id: "",
        offer_generation: OfferGeneration(u64::MAX),
        execution_regions: &[],
        execution_fusions: &[],
    }
}
fn tail() -> FragmentFingerprintTailRef<'static> {
    static BUDGET: SignStorageBudget = SignStorageBudget {
        item_capacity: 0,
        byte_capacity: 0,
    };
    FragmentFingerprintTailRef {
        connections: PlannedConnectionRefs::Borrowed(&[]),
        fore_ports: &[],
        shared_pools: &[],
        startup_dependencies: StartupDependencyRefs::Borrowed(&[]),
        startup_order: PlacementIdRefs::Borrowed(&[]),
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: ExpectedTerminalRefs::Borrowed(&[]),
        expected_sign: ExpectedSignRefs::Borrowed(&[]),
        sign_storage_budget: &BUDGET,
        states: &[],
    }
}
#[test]
fn fallible_fingerprint_outputs_match_original_ids_and_retry_after_null() {
    let expected_root = compute_plan_fingerprint_ref(root());
    let expected_fragment = FragmentFingerprintBuilder::new_ref(header(), 0)
        .finish_ref(tail())
        .unwrap();
    assert_eq!(
        try_compute_plan_fingerprint_ref(root()).unwrap(),
        expected_root
    );
    assert_eq!(
        FragmentFingerprintBuilder::new_ref(header(), 0)
            .try_finish_ref(tail())
            .unwrap(),
        expected_fragment
    );
    let (result, memory) = allocation_probe::observe(|| {
        let _refusal = allocation_probe::refuse_next_allocation_size(64);
        try_compute_plan_fingerprint_ref(root())
    });
    assert!(result.is_err());
    assert_eq!(memory.retained_bytes(), 0);
    let (result, memory) = allocation_probe::observe(|| {
        let _refusal = allocation_probe::refuse_next_allocation_size(64);
        FragmentFingerprintBuilder::new_ref(header(), 0).try_finish_ref(tail())
    });
    assert_eq!(result, Err(FragmentFingerprintPreparationRefusal::Capacity));
    assert_eq!(memory.retained_bytes(), 0);
    assert_eq!(
        try_compute_plan_fingerprint_ref(root()).unwrap(),
        expected_root
    );
    assert_eq!(
        FragmentFingerprintBuilder::new_ref(header(), 0)
            .try_finish_ref(tail())
            .unwrap(),
        expected_fragment
    );
}
#[test]
fn incomplete_fragment_refuses_before_requesting_any_output_storage() {
    let (result, memory) = allocation_probe::observe(|| {
        let _refusal = allocation_probe::refuse_next_allocation_size(64);
        FragmentFingerprintBuilder::new_ref(header(), 1).try_finish_ref(tail())
    });
    assert_eq!(
        result,
        Err(FragmentFingerprintPreparationRefusal::Fingerprint(
            FragmentFingerprintRefusal::IncompletePlacements
        ))
    );
    assert_eq!(memory.allocations + memory.reallocations, 0);
}
