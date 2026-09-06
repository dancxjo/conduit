use std::{cell::RefCell, rc::Rc};

use conduit_core::{
    mandatory_sign_storage_requirement, prepare_plan_on_hosts, seal_plan, start_prepared_plan,
    ActivePlayId, BootId, CancellationPolicy, CheckedFormId, ExpandedFormId, ExpectedSign,
    ExpectedTerminal, FormIdentity, FragmentId, HostId, HostPreparationRefusal, OfferGeneration,
    Plan, PlanFragment, PlanPreparationError, PlanPreparationHost, PreparationHostIdentity,
    PreparedFragmentReceipt, SignStorageBudget, SourceDocumentId, TerminalPolicy,
};

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
            plan_id: conduit_core::PlanId::from(""),
            fragment_id: FragmentId::from(""),
            source_document_id: SourceDocumentId::from("source"),
            checked_form_id: CheckedFormId::from("checked"),
            expanded_form_id: ExpandedFormId::from(label),
            realization_backs: vec![],
            host_id: HostId::from(*host),
            boot_id: BootId::from(format!("{host}-boot")),
            offer_generation: OfferGeneration(7),
            placements: vec![],
            execution_regions: vec![],
            execution_fusions: vec![],
            states: Vec::new(),
            connections: vec![],
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
        FormIdentity {
            source_document_id: SourceDocumentId::from("source"),
            checked_form_id: CheckedFormId::from("checked"),
            expanded_form_id: ExpandedFormId::from(label),
        },
        fragments,
    )
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
