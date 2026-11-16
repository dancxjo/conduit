use super::*;
use alloc::format;

fn admitted_viewer() -> (BodyLifecycleSession, MembershipCredential) {
    let mut session = session();
    let (mut manager, mut inventory, proof, _) = pending(&session);
    let member = session
        .admit_ambient_host(&mut manager, &mut inventory, &proof, 110, &host(), &boot())
        .unwrap();
    (session, member)
}

fn propose_empty_workset(session: &mut BodyLifecycleSession) {
    let plans = session
        .evidence
        .body
        .workset
        .plots()
        .iter()
        .map(|plot| crate::BodyPlotPlan {
            plot: plot.clone(),
            plan: conduit_core::seal_plan(
                conduit_core::PlotIdentity {
                    source_document_id: plot.source_document_id.clone(),
                    checked_plot_id: plot.checked_plot_id.clone(),
                    expanded_plot_id: "expanded/test".into(),
                },
                vec![],
            ),
        })
        .collect();
    session.propose(plans, &host(), &boot()).unwrap();
}

#[test]
fn unused_member_query_preserves_pending_realization_and_accepts_same_boot() {
    let (mut session, member) = admitted_viewer();
    propose_empty_workset(&mut session);
    let evidence = session.evidence.clone();
    let realization = session.realization.clone();
    let foreground = session.foreground.clone();
    for boot_id in ["boot/browser/1", "boot/browser/2"] {
        assert_eq!(
            session.check_unused_member_return(
                &member.part_id,
                &advertisement(boot_id),
                &host(),
                &boot()
            ),
            Ok(())
        );
    }
    assert_eq!(session.evidence, evidence);
    assert_eq!(session.realization, realization);
    assert_eq!(session.foreground, foreground);
    // Eligibility is not admission authority: the existing gate still refuses.
    let (mut manager, mut inventory, proof, _) = pending(&session);
    assert_eq!(
        session.admit_ambient_host(&mut manager, &mut inventory, &proof, 110, &host(), &boot()),
        Err(Error::NotLulled)
    );
}

#[test]
fn unused_member_query_requires_actual_prior_binding_and_current_authority() {
    let (mut session, member) = admitted_viewer();
    assert_eq!(
        session.check_unused_member_return(
            &member.part_id,
            &advertisement("boot/browser/2"),
            &host(),
            &"boot/stale".into()
        ),
        Err(Error::StaleHost)
    );
    let mut foreign = advertisement("boot/browser/2");
    foreign.host_id = "host/foreign".into();
    assert_eq!(
        session.check_unused_member_return(&member.part_id, &foreign, &host(), &boot()),
        Err(Error::StaleHost)
    );
    session
        .observe_host_lost(&member.host_id, &member.boot_id, &host(), &boot())
        .unwrap();
    assert_eq!(
        session.check_unused_member_return(
            &member.part_id,
            &advertisement("boot/browser/2"),
            &host(),
            &boot()
        ),
        Err(Error::StaleHost)
    );
}

#[test]
fn typed_child_and_mask_dependencies_refuse_without_mutating_the_realization() {
    // Structural scanner fixtures, not execution or admission evidence.
    let (mut session, member) = admitted_viewer();
    propose_empty_workset(&mut session);
    let empty = session.realization.as_ref().unwrap().plan.plots[0]
        .plan
        .clone();
    let binding = conduit_core::PlannedActivationPreparationBinding {
        activation_id: "activation/test".into(),
        owner_placement_id: "placement/test".into(),
        owner_fragment_id: "fragment/test".into(),
        owner_host_id: host(),
        owner_boot_id: boot(),
        owner_offer_generation: OfferGeneration(1),
        selected_plan_id: empty.plan_id.clone(),
        child_fragments: vec![conduit_core::PlannedSubordinateFragmentBinding {
            fragment_id: "fragment/child".into(),
            host_id: member.host_id.clone(),
            boot_id: "boot/older".into(),
            offer_generation: OfferGeneration(1),
            obligation_digest: [0; 32],
        }],
    };
    let mut dependent = empty.clone();
    dependent.activation_preparations.push(binding);
    assert!(!plan_references_host(&empty, &member.host_id, 0));
    assert!(plan_references_host(&dependent, &member.host_id, 0));
    assert!(plan_references_host(
        &empty,
        &member.host_id,
        conduit_core::MAXIMUM_PLANNED_ACTIVATION_DEPTH + 1
    ));
    session
        .realization
        .as_mut()
        .unwrap()
        .plan
        .mask_topologies
        .push(crate::BodyMaskTopology {
            face: crate::BodyFaceSelector {
                plot: None,
                source_placement_id: None,
            },
            chains: vec![crate::BodyMaskChainPlan {
                plan: dependent,
                stage_placement_ids: vec![],
            }],
        });
    let before = session.realization.clone();
    assert_eq!(
        session.check_unused_member_return(
            &member.part_id,
            &advertisement("boot/browser/2"),
            &host(),
            &boot()
        ),
        Err(Error::NotLulled)
    );
    assert_eq!(session.realization, before);
}

// Synthetic sealed scanner fixture; no execution readiness claim.
fn structural_plan(label: &str, host: &str) -> Plan {
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

#[test]
fn plot_placement_reference_refuses_even_when_fragment_owner_differs() {
    let (mut session, member) = admitted_viewer();
    propose_empty_workset(&mut session);
    let mut plan = structural_plan("scanner", "host/other");
    assert!(!plan_references_host(&plan, &member.host_id, 0));
    plan.fragments[0].placements[0].host_id = member.host_id.clone();
    assert!(plan_references_host(&plan, &member.host_id, 0));
    session.realization.as_mut().unwrap().plan.plots[0].plan = plan;
    let before = session.realization.clone();
    assert_eq!(
        session.check_unused_member_return(
            &member.part_id,
            &advertisement("boot/browser/2"),
            &host(),
            &boot()
        ),
        Err(Error::NotLulled)
    );
    assert_eq!(session.realization, before);
}

#[test]
fn unused_return_query_leaves_started_play_identity_exact() {
    let (mut session, member) = admitted_viewer();
    propose_empty_workset(&mut session);
    let current = session.realization.as_ref().unwrap().clone();
    let play = crate::BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |sequence| bind_sign(&host(), &boot(), Some(&play.active_play_id), sequence).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, sign(1))
        .unwrap();
    session.started(&host(), &boot(), play, wake).unwrap();
    let before = session.realization.clone();
    let evidence = session.evidence.clone();
    assert_eq!(
        session.check_unused_member_return(
            &member.part_id,
            &advertisement("boot/browser/2"),
            &host(),
            &boot()
        ),
        Ok(())
    );
    assert_eq!(session.realization, before);
    assert_eq!(session.evidence, evidence);
}
