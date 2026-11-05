//! Synthetic sealed Plans test containment, never execution or device readiness.
use conduit_core::*;

pub fn plan(label: &str, host: &str) -> Plan {
    let expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlanTerminal,
    ];
    let fragment = PlanFragment {
        completion_policy: conduit_core::PlanCompletionPolicy::Live,
        plan_id: PlanId::from(""),
        fragment_id: FragmentId::from(""),
        source_document_id: SourceDocumentId::from(format!("source/{label}")),
        checked_plot_id: CheckedPlotId::from(format!("checked/{label}")),
        expanded_plot_id: ExpandedPlotId::from(label),
        realization_backs: Vec::new(),
        host_id: HostId::from(host),
        boot_id: BootId::from(format!("{host}-boot")),
        offer_generation: OfferGeneration(7),
        placements: vec![conduit_core::planned_gear_from_parts! {
            semantic_contract: Default::default(),
            placement_id: PlacementId::from(format!("{label}-placement")),
            gear_id: GearId::from("gear-a"),
            kind_id: KindId::from("test/kind"),
            kind_contract_revision: KindIdentity::from("test/kind@1"),
            execution_profile_id: ExecutionProfileId::from("test/hosted@1"),
            configuration: vec![],
            host_id: HostId::from(host),
            boot_id: BootId::from(format!("{host}-boot")),
            offer_generation: OfferGeneration(7),
            capability_id: CapabilityId::from(format!("{host}/capability")),
            implementation_id: ImplementationId::from(format!("{host}/implementation")),
            artifact_id: ArtifactId::from(format!("{host}/artifact")),
            base: None,
            realization_characteristics: vec![],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 16,
            },
            inputs: vec![],
            outputs: vec![],
            terminal_transductions: Vec::new(),
            host_calls: vec![],
            resources: vec![ResourceBinding {
                content: None,
                pool_id: ResourcePoolId::from(format!("{host}/timer-pool")),
                class_id: ResourceClassId::from("test/timer"),
                units: 1,
                protected: None,
                compute: None,
            }],
            authority: vec![],
            pool_references: vec![],
        }],
        execution_regions: vec![],
        execution_fusions: vec![],
        states: Vec::new(),
        connections: vec![],
        fore_ports: vec![],
        shared_pools: vec![],
        startup_dependencies: vec![],
        startup_order: vec![],
        cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
        terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
        expected_terminals: vec![ExpectedTerminal::PlanCompleted],
        expected_sign: expected_sign.clone(),
        sign_storage_budget: mandatory_sign_storage_requirement(&expected_sign).unwrap_or(
            SignStorageBudget {
                item_capacity: 0,
                byte_capacity: 0,
            },
        ),
        plan_fragments: vec![],
    };
    seal_plan(
        PlotIdentity {
            source_document_id: SourceDocumentId::from(format!("source/{label}")),
            checked_plot_id: CheckedPlotId::from(format!("checked/{label}")),
            expanded_plot_id: ExpandedPlotId::from(label),
        },
        vec![fragment],
    )
}
