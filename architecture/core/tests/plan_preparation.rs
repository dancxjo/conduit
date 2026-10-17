use std::{cell::RefCell, rc::Rc};

use conduit_core::{
    mandatory_sign_storage_requirement, prepare_plan_on_hosts, seal_plan, start_prepared_plan,
    verify_prepared_plan, ActivePlayId, BootId, CancellationPolicy, CheckedPlotId, ExpandedPlotId,
    ExpectedSign, ExpectedTerminal, FragmentId, HostId, HostPreparationRefusal, OfferGeneration,
    Plan, PlanFragment, PlanPreparationError, PlanPreparationHost, PlotIdentity,
    PreparationHostIdentity, PreparedFragmentReceipt, SignStorageBudget, SourceDocumentId,
    TerminalPolicy,
};
#[path = "common/sealed_state.rs"]
mod common;

struct MultiHost {
    identity: PreparationHostIdentity,
    prepared: Vec<PreparedFragmentReceipt>,
    fail_on: Option<usize>,
    stale_on: Option<usize>,
    preparations: usize,
    releases: usize,
    release_failure: Option<HostPreparationRefusal>,
}

impl MultiHost {
    fn new() -> Self {
        Self {
            identity: PreparationHostIdentity {
                host_id: HostId::from("host"),
                boot_id: BootId::from("boot"),
                offer_generation: OfferGeneration(1),
            },
            prepared: vec![],
            fail_on: None,
            stale_on: None,
            preparations: 0,
            releases: 0,
            release_failure: None,
        }
    }
}

impl PlanPreparationHost for MultiHost {
    fn preparation_identity(&self) -> PreparationHostIdentity {
        self.identity.clone()
    }
    fn prepare_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<PreparedFragmentReceipt, HostPreparationRefusal> {
        self.preparations += 1;
        if self.fail_on == Some(self.preparations) {
            return Err(HostPreparationRefusal::ResourceUnavailable);
        }
        let receipt = if self.stale_on == Some(self.preparations) {
            let mut stale = fragment.clone();
            stale.offer_generation = OfferGeneration(fragment.offer_generation.0 + 1);
            PreparedFragmentReceipt::new(&stale)
        } else {
            PreparedFragmentReceipt::new(fragment)
        };
        self.prepared.push(receipt.clone());
        Ok(receipt)
    }
    fn release_fragment(
        &mut self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        if let Some(reason) = self.release_failure {
            return Err(reason);
        }
        let index = self
            .prepared
            .iter()
            .position(|value| value == receipt)
            .ok_or(HostPreparationRefusal::PreparedBindingMismatch)?;
        self.prepared.remove(index);
        self.releases += 1;
        Ok(())
    }
    fn validate_start(
        &self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        self.prepared
            .contains(receipt)
            .then_some(())
            .ok_or(HostPreparationRefusal::PreparedBindingMismatch)
    }
    fn start_fragment(&mut self, _: &PreparedFragmentReceipt) -> ActivePlayId {
        ActivePlayId::from("play")
    }
}

fn activation_plan() -> Plan {
    let mut child_fragment = common::fragment();
    child_fragment.fore_ports = vec![
        conduit_core::PlannedForePort {
            front_port_id: conduit_core::port_id("in"),
            direction: conduit_core::PortDirection::Input,
            placement_id: conduit_core::PlacementId::from("placement"),
            gear_port_id: conduit_core::port_id("next"),
            value_kind: conduit_core::kind_id("fixture/byte@1"),
            value_contract: None,
            abnormal_kind: None,
            track: conduit_core::ConnectionTrack::Payload,
            temporal: conduit_core::PortTemporal::Value,
            pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
            selected_line: None,
        },
        conduit_core::PlannedForePort {
            front_port_id: conduit_core::port_id("out"),
            direction: conduit_core::PortDirection::Output,
            placement_id: conduit_core::PlacementId::from("placement"),
            gear_port_id: conduit_core::port_id("current"),
            value_kind: conduit_core::kind_id("fixture/byte@1"),
            value_contract: None,
            abnormal_kind: None,
            track: conduit_core::ConnectionTrack::Payload,
            temporal: conduit_core::PortTemporal::Value,
            pressure_policy: conduit_core::DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: 1,
            selected_line: None,
        },
    ];
    let child = common::seal(child_fragment);
    let outer = common::fragment();
    let activation = conduit_core::PlannedActivation {
        activation_id: "each".into(),
        owner_placement_id: conduit_core::PlacementId::from("placement"),
        selected_plan_id: child.plan_id.clone(),
        selected_plan: Box::new(child),
        input: conduit_core::PlannedActivationFront {
            front_port_id: conduit_core::port_id("in"),
            value_kind: conduit_core::kind_id("fixture/byte@1"),
            abnormal_kind: None,
        },
        output: conduit_core::PlannedActivationFront {
            front_port_id: conduit_core::port_id("out"),
            value_kind: conduit_core::kind_id("fixture/byte@1"),
            abnormal_kind: None,
        },
        limits: conduit_core::PlannedActivationLimits {
            maximum_active: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 1,
            maximum_items: 1,
        },
        terminal_policy: conduit_core::PlannedActivationTerminalPolicy::DrainThenPropagateExact,
        cancellation_policy:
            conduit_core::PlannedActivationCancellationPolicy::CancelActiveAndRejectLateCompletion,
        effect_multiplicity:
            conduit_core::PlannedActivationEffectMultiplicity::OncePerAcceptedInput,
        per_activation_sign_budget: SignStorageBudget {
            item_capacity: 2,
            byte_capacity: 64,
        },
    };
    conduit_core::seal_plan_with_activations(
        PlotIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_plot_id: outer.checked_plot_id.clone(),
            expanded_plot_id: outer.expanded_plot_id.clone(),
        },
        conduit_core::PlanCompletionPolicy::Live,
        vec![],
        vec![activation],
        vec![outer],
    )
}

#[test]
fn subordinate_fragments_are_prepared_and_rollback_atomically() {
    let plan = activation_plan();
    let mut host = MultiHost::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
    assert_eq!(prepared.receipts().len(), 1);
    assert_eq!(prepared.subordinate_receipts().len(), 1);
    assert!(verify_prepared_plan(&prepared, &plan));
    assert_eq!(host.preparations, 2);

    let mut failing = MultiHost::new();
    failing.fail_on = Some(2);
    assert!(matches!(
        prepare_plan_on_hosts(&plan, &mut [&mut failing]),
        Err(PlanPreparationError::HostRefused { .. })
    ));
    assert!(failing.prepared.is_empty());
    assert_eq!(failing.releases, 1);

    let mut stale = MultiHost::new();
    stale.stale_on = Some(2);
    assert!(matches!(
        prepare_plan_on_hosts(&plan, &mut [&mut stale]),
        Err(PlanPreparationError::InvalidReceipt { .. })
    ));
    assert!(stale.prepared.is_empty());
    assert_eq!(stale.releases, 2);
}

#[test]
fn subordinate_start_refuses_stale_boot_and_offer_before_any_play() {
    let plan = activation_plan();
    let mut stale_boot = MultiHost::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut stale_boot]).unwrap();
    let child = prepared.subordinate_receipts()[0].1.fragment_id().clone();
    stale_boot.identity.boot_id = BootId::from("new-boot");
    assert!(matches!(
        start_prepared_plan(prepared, &mut [&mut stale_boot]),
        Err(PlanPreparationError::StartRefused {
            fragment_id,
            reason: HostPreparationRefusal::StaleBoot,
        }) if fragment_id == child
    ));

    let mut stale_offer = MultiHost::new();
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut stale_offer]).unwrap();
    let child = prepared.subordinate_receipts()[0].1.fragment_id().clone();
    stale_offer.identity.offer_generation = OfferGeneration(2);
    assert!(matches!(
        start_prepared_plan(prepared, &mut [&mut stale_offer]),
        Err(PlanPreparationError::StartRefused {
            fragment_id,
            reason: HostPreparationRefusal::StaleOffer,
        }) if fragment_id == child
    ));
}

#[test]
fn invalid_subordinate_receipt_preserves_its_release_refusal() {
    let plan = activation_plan();
    let mut host = MultiHost::new();
    host.stale_on = Some(2);
    host.release_failure = Some(HostPreparationRefusal::LocalFailure(
        conduit_core::FailureReason::ResourceCapacityExceeded,
    ));
    assert!(matches!(
        prepare_plan_on_hosts(&plan, &mut [&mut host]),
        Err(PlanPreparationError::InvalidReceipt {
            rollback_failures,
            ..
        }) if rollback_failures.iter().any(|failure| failure.reason
            == HostPreparationRefusal::LocalFailure(
                conduit_core::FailureReason::ResourceCapacityExceeded
            ))
    ));
}

#[test]
fn subordinate_preparation_total_is_finitely_bounded_before_host_work() {
    let base = activation_plan();
    let mut outer = base.fragments[0].clone();
    let template = outer.placements[0].clone();
    let template_activation = base.activations[0].clone();
    let mut activations = Vec::new();
    for index in 0..32 {
        let placement_id = conduit_core::PlacementId::from(format!("activation-{index}"));
        let mut placement = template.clone();
        placement.placement_id = placement_id.clone();
        placement.gear_id = conduit_core::GearId::from(format!("activation-{index}"));
        outer.placements.push(placement);
        outer.startup_order.push(placement_id.clone());
        let conduit_core::PlannedActivationEntry::Unary(mut activation) =
            template_activation.clone()
        else {
            unreachable!()
        };
        activation.activation_id = format!("activation-{index}");
        activation.owner_placement_id = placement_id;
        activations.push(conduit_core::PlannedActivationEntry::Unary(activation));
    }
    let plan = conduit_core::seal_plan_with_activation_entries(
        PlotIdentity {
            source_document_id: outer.source_document_id.clone(),
            checked_plot_id: outer.checked_plot_id.clone(),
            expanded_plot_id: outer.expanded_plot_id.clone(),
        },
        conduit_core::PlanCompletionPolicy::Live,
        vec![],
        activations,
        vec![outer],
    );
    assert!(conduit_core::verify_plan(&plan));
    let mut host = MultiHost::new();
    assert_eq!(
        prepare_plan_on_hosts(&plan, &mut [&mut host]),
        Err(PlanPreparationError::HostCapacityExceeded)
    );
    assert_eq!(host.preparations, 0);
}

struct TestHost {
    identity: PreparationHostIdentity,
    prepared: Option<PreparedFragmentReceipt>,
    refusal: Option<HostPreparationRefusal>,
    release_failure: Option<HostPreparationRefusal>,
    preparations: u8,
    releases: u8,
    starts: u8,
    semantic_effects: u8,
    release_log: Rc<RefCell<Vec<String>>>,
}

impl TestHost {
    fn new(host: &str, release_log: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            identity: PreparationHostIdentity {
                host_id: HostId::from(host),
                boot_id: BootId::from(format!("{host}-boot")),
                offer_generation: OfferGeneration(7),
            },
            prepared: None,
            refusal: None,
            release_failure: None,
            preparations: 0,
            releases: 0,
            starts: 0,
            semantic_effects: 0,
            release_log,
        }
    }
}

impl PlanPreparationHost for TestHost {
    fn preparation_identity(&self) -> PreparationHostIdentity {
        self.identity.clone()
    }

    fn prepare_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<PreparedFragmentReceipt, HostPreparationRefusal> {
        self.preparations += 1;
        if let Some(reason) = self.refusal {
            return Err(reason);
        }
        if self.prepared.is_some() {
            return Err(HostPreparationRefusal::AlreadyPrepared);
        }
        let receipt = PreparedFragmentReceipt::new(fragment);
        self.prepared = Some(receipt.clone());
        Ok(receipt)
    }

    fn release_fragment(
        &mut self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        if self.prepared.as_ref() != Some(receipt) {
            return Err(HostPreparationRefusal::PreparedBindingMismatch);
        }
        if let Some(reason) = self.release_failure {
            return Err(reason);
        }
        self.prepared = None;
        self.releases += 1;
        self.release_log
            .borrow_mut()
            .push(self.identity.host_id.as_str().to_owned());
        Ok(())
    }

    fn validate_start(
        &self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        if self.prepared.as_ref() != Some(receipt) {
            return Err(HostPreparationRefusal::PreparedBindingMismatch);
        }
        if receipt.host() != &self.identity {
            return Err(HostPreparationRefusal::PreparedBindingMismatch);
        }
        Ok(())
    }

    fn start_fragment(&mut self, receipt: &PreparedFragmentReceipt) -> ActivePlayId {
        assert_eq!(self.validate_start(receipt), Ok(()));
        self.starts += 1;
        self.semantic_effects += 1;
        ActivePlayId::from(format!(
            "{}/play/{}",
            self.identity.host_id.as_str(),
            self.starts
        ))
    }
}

fn exact_plan(hosts: &[&str], label: &str) -> Plan {
    let expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlanTerminal,
    ];
    let fragments = hosts
        .iter()
        .map(|host| PlanFragment {
            completion_policy: conduit_core::PlanCompletionPolicy::Live,
            plan_id: conduit_core::PlanId::from(""),
            fragment_id: FragmentId::from(""),
            source_document_id: SourceDocumentId::from("source"),
            checked_plot_id: CheckedPlotId::from("checked"),
            expanded_plot_id: ExpandedPlotId::from(label),
            realization_backs: vec![],
            host_id: HostId::from(*host),
            boot_id: BootId::from(format!("{host}-boot")),
            offer_generation: OfferGeneration(7),
            placements: vec![],
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
        })
        .collect();
    seal_plan(
        PlotIdentity {
            source_document_id: SourceDocumentId::from("source"),
            checked_plot_id: CheckedPlotId::from("checked"),
            expanded_plot_id: ExpandedPlotId::from(label),
        },
        fragments,
    )
}

#[test]
fn completion_policy_is_sealed_into_plan_and_fragment_identity() {
    let live = exact_plan(&["origin"], "completion-policy");
    let semantic = conduit_core::seal_plan_with_completion(
        PlotIdentity {
            source_document_id: live.source_document_id.clone(),
            checked_plot_id: live.checked_plot_id.clone(),
            expanded_plot_id: live.expanded_plot_id.clone(),
        },
        conduit_core::PlanCompletionPolicy::SemanticCompletion,
        live.fragments.clone(),
    );

    assert_ne!(semantic.plan_id, live.plan_id);
    assert_eq!(
        semantic.completion_policy,
        conduit_core::PlanCompletionPolicy::SemanticCompletion
    );
    assert!(semantic.fragments.iter().all(|fragment| {
        fragment.completion_policy == conduit_core::PlanCompletionPolicy::SemanticCompletion
    }));
    assert!(conduit_core::verify_plan(&semantic));

    let mut mutated = semantic.clone();
    mutated.completion_policy = conduit_core::PlanCompletionPolicy::Live;
    assert!(!conduit_core::verify_plan(&mutated));
}

#[test]
fn every_selected_host_prepares_before_any_exact_fragment_starts() {
    let plan = exact_plan(&["hosted", "browser", "constrained"], "heterogeneous");
    let log = Rc::new(RefCell::new(vec![]));
    let mut hosted = TestHost::new("hosted", log.clone());
    let mut browser = TestHost::new("browser", log.clone());
    let mut constrained = TestHost::new("constrained", log);

    let prepared =
        prepare_plan_on_hosts(&plan, &mut [&mut hosted, &mut browser, &mut constrained]).unwrap();
    assert_eq!(prepared.receipts().len(), 3);
    assert_eq!(
        (
            hosted.preparations,
            browser.preparations,
            constrained.preparations
        ),
        (1, 1, 1)
    );
    assert_eq!(
        (hosted.starts, browser.starts, constrained.starts),
        (0, 0, 0)
    );
    assert_eq!(
        (
            hosted.semantic_effects,
            browser.semantic_effects,
            constrained.semantic_effects
        ),
        (0, 0, 0)
    );
    let substituted_plan = exact_plan(&["hosted"], "substituted-plan");
    let substituted_receipt = PreparedFragmentReceipt::new(&substituted_plan.fragments[0]);
    assert_eq!(
        hosted.validate_start(&substituted_receipt),
        Err(HostPreparationRefusal::PreparedBindingMismatch)
    );

    let started =
        start_prepared_plan(prepared, &mut [&mut hosted, &mut browser, &mut constrained]).unwrap();
    assert_eq!(started.plan_id(), &plan.plan_id);
    assert_eq!(started.active_plays().len(), 3);
    assert_eq!(
        (hosted.starts, browser.starts, constrained.starts),
        (1, 1, 1)
    );
}

#[test]
fn stale_offer_missing_host_and_finite_bound_refuse_before_start() {
    let plan = exact_plan(&["origin", "device"], "identity-refusals");
    let log = Rc::new(RefCell::new(vec![]));
    let mut origin = TestHost::new("origin", log.clone());
    let missing_error = prepare_plan_on_hosts(&plan, &mut [&mut origin]).unwrap_err();
    assert!(matches!(
        missing_error,
        PlanPreparationError::HostSelectionFailed {
            reason: conduit_core::HostSelectionFailure::Missing,
            rollback_failures,
            ..
        } if rollback_failures.is_empty()
    ));
    assert!(origin.prepared.is_none());

    let mut current_origin = TestHost::new("origin", log.clone());
    let mut stale_device = TestHost::new("device", log);
    stale_device.identity.offer_generation = OfferGeneration(8);
    let stale_error =
        prepare_plan_on_hosts(&plan, &mut [&mut current_origin, &mut stale_device]).unwrap_err();
    assert!(matches!(
        stale_error,
        PlanPreparationError::HostRefused {
            reason: HostPreparationRefusal::StaleOffer,
            rollback_failures,
            ..
        } if rollback_failures.is_empty()
    ));
    assert_eq!((current_origin.starts, stale_device.starts), (0, 0));
    assert!(current_origin.prepared.is_none());

    let host_names = (0..=conduit_core::MAX_PREPARATION_HOSTS)
        .map(|index| format!("host-{index}"))
        .collect::<Vec<_>>();
    let host_refs = host_names.iter().map(String::as_str).collect::<Vec<_>>();
    let oversized = exact_plan(&host_refs, "oversized");
    assert_eq!(
        prepare_plan_on_hosts(&oversized, &mut []),
        Err(PlanPreparationError::HostCapacityExceeded)
    );
}

#[test]
fn one_host_veto_rolls_back_prior_reservations_and_a_later_attempt_succeeds() {
    let plan = exact_plan(&["origin", "browser", "device"], "veto-retry");
    let log = Rc::new(RefCell::new(vec![]));
    let mut origin = TestHost::new("origin", log.clone());
    let mut browser = TestHost::new("browser", log.clone());
    let mut device = TestHost::new("device", log.clone());
    device.refusal = Some(HostPreparationRefusal::ResourceUnavailable);

    let error =
        prepare_plan_on_hosts(&plan, &mut [&mut origin, &mut browser, &mut device]).unwrap_err();
    assert!(matches!(
        error,
        PlanPreparationError::HostRefused {
            reason: HostPreparationRefusal::ResourceUnavailable,
            rollback_failures,
            ..
        } if rollback_failures.is_empty()
    ));
    assert_eq!(&*log.borrow(), &["browser", "origin"]);
    assert!(origin.prepared.is_none() && browser.prepared.is_none());
    assert_eq!((origin.starts, browser.starts, device.starts), (0, 0, 0));

    device.refusal = None;
    let prepared =
        prepare_plan_on_hosts(&plan, &mut [&mut origin, &mut browser, &mut device]).unwrap();
    start_prepared_plan(prepared, &mut [&mut origin, &mut browser, &mut device]).unwrap();
    assert_eq!((origin.starts, browser.starts, device.starts), (1, 1, 1));
}

#[test]
fn stale_identity_and_failed_release_remain_distinct_machine_readable_evidence() {
    let plan = exact_plan(&["origin", "device"], "stale");
    let log = Rc::new(RefCell::new(vec![]));
    let mut origin = TestHost::new("origin", log.clone());
    let mut device = TestHost::new("device", log);
    let prepared = prepare_plan_on_hosts(&plan, &mut [&mut origin, &mut device]).unwrap();

    device.identity.boot_id = BootId::from("device-new-boot");
    let error = start_prepared_plan(prepared, &mut [&mut origin, &mut device]).unwrap_err();
    assert!(matches!(
        error,
        PlanPreparationError::StartRefused {
            reason: HostPreparationRefusal::StaleBoot,
            ..
        }
    ));
    assert_eq!((origin.starts, device.starts), (0, 0));

    let rollback_plan = exact_plan(&["origin", "missing"], "rollback-failure");
    let log = Rc::new(RefCell::new(vec![]));
    let mut rollback_origin = TestHost::new("origin", log);
    rollback_origin.release_failure = Some(HostPreparationRefusal::LocalFailure(
        conduit_core::FailureReason::ResourceCapacityExceeded,
    ));
    let error = prepare_plan_on_hosts(&rollback_plan, &mut [&mut rollback_origin]).unwrap_err();
    assert!(matches!(
        error,
        PlanPreparationError::HostSelectionFailed {
            reason: conduit_core::HostSelectionFailure::Missing,
            rollback_failures,
            ..
        } if rollback_failures.len() == 1
            && rollback_failures[0].reason == HostPreparationRefusal::LocalFailure(
                conduit_core::FailureReason::ResourceCapacityExceeded
            )
    ));
}
