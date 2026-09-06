//! Acquired-provider conformance; no physical hardware claim.
use conduit_core::{resource_offer, Plan, PlanId, INPUT_RESOURCE_CLASS};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::{
    hosted_indicator::{
        HostedIndicatorAdapter, IndicatorBinding, IndicatorFailure, IndicatorRequest,
    },
    hosted_keyboard::{HostedKeyboardAdapter, HostedKeyboardPoll},
    HostedRunAdapters, RunControl, StdHost, TimerAdapter,
};

struct Keyboard(usize);
impl HostedKeyboardAdapter for Keyboard {
    fn poll_next(&mut self) -> HostedKeyboardPoll {
        let phase = self.0;
        self.0 += 1;
        if phase < 2 {
            HostedKeyboardPoll::Event(
                conduit_human::KeyEvent::decode(&[0x2c, phase as u8, 0]).unwrap(),
            )
        } else {
            HostedKeyboardPoll::Cancelled
        }
    }
}
struct Timer;
impl TimerAdapter for Timer {
    fn wait(&mut self, _: std::time::Duration) {}
}

struct Provider {
    binding: IndicatorBinding,
    plan: PlanId,
    calls: usize,
    states: [bool; 2],
    failure: Option<IndicatorFailure>,
    replace_after_first: bool,
}
impl HostedIndicatorAdapter for Provider {
    fn binding(&self) -> &IndicatorBinding {
        &self.binding
    }
    fn present(&mut self, request: IndicatorRequest<'_>) -> Result<(), IndicatorFailure> {
        assert_eq!(request.play.plan_id, self.plan);
        assert_eq!(request.play.host_id, self.binding.host_id);
        assert_eq!(request.play.boot_id, self.binding.boot_id);
        assert_eq!(request.request.0 as usize, self.calls);
        self.states[self.calls] = request.state.get();
        self.calls += 1;
        if self.replace_after_first {
            self.binding.boot_id = "replacement-boot".into();
        }
        self.failure.map_or(Ok(()), Err)
    }
}

fn prepare() -> (StdHost, Plan, Provider) {
    let mut advertisement = StdHost::new().advertisement().clone();
    advertisement.capabilities = vec![
        conduit_std_offers::button::offer(),
        conduit_std_offers::button::mapper_offer(),
        conduit_std_offers::indicator_resource::offer(),
    ];
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement.resources.extend([
        resource_offer("proof/keyboard", INPUT_RESOURCE_CLASS, 1),
        resource_offer(
            "proof/indicator",
            conduit_std_offers::indicator_resource::RESOURCE_CLASS,
            1,
        ),
    ]);
    advertisement.resources.sort();
    let binding = IndicatorBinding {
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        offer_generation: advertisement.offer_generation,
        pool_id: "proof/indicator".into(),
    };
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)
        .unwrap();
    let source = include_str!("../../../forms/button-across-room/main.conduit");
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let form = expand_canonical_form(&checked, "button_across_room", &profile).unwrap();
    let hosts = [host.advertisement().clone()];
    let choices = conduit_planner::default_expanded_placements(&form, &hosts).unwrap();
    let limits = form
        .connections
        .iter()
        .map(|c| {
            (
                (
                    c.source_gear_id.clone(),
                    c.source_port_id.clone(),
                    c.sink_gear_id.clone(),
                    c.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: if c.value_kind.as_str() == conduit_core::BOOL_INFO_ID {
                        1
                    } else {
                        conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES
                    },
                },
            )
        })
        .collect();
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &form,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &Default::default(),
            line_candidates: &Default::default(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
    .unwrap();
    let provider = Provider {
        binding,
        plan: plan.plan_id.clone(),
        calls: 0,
        states: [false; 2],
        failure: None,
        replace_after_first: false,
    };
    (host, plan, provider)
}

#[test]
fn canonical_form_completes_only_through_exact_indicator_adapter() {
    let (mut host, plan, mut provider) = prepare();
    let mut output = Vec::new();
    let report = host
        .run_fragment_controlled_with_adapters_to(
            plan.fragments[0].clone(),
            &mut output,
            &mut Timer,
            &RunControl::default(),
            HostedRunAdapters {
                keyboard: Some(&mut Keyboard(0)),
                indicator: Some(&mut provider),
            },
        )
        .unwrap();
    assert_eq!(provider.calls, 2);
    assert_eq!(provider.states, [true, false]);
    assert!(!String::from_utf8(output).unwrap().contains("bool value="));
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Completed
        })
    ));
}

#[test]
fn missing_or_stale_adapter_refuses_before_input_is_polled() {
    for mismatch in 0..5 {
        let (mut host, plan, mut provider) = prepare();
        match mismatch {
            0 => provider.binding.host_id = "wrong-host".into(),
            1 => provider.binding.boot_id = "wrong-boot".into(),
            2 => provider.binding.offer_generation.0 += 1,
            3 => provider.binding.pool_id = "wrong-pool".into(),
            _ => {}
        }
        let mut keyboard = Keyboard(0);
        let error = host
            .run_fragment_controlled_with_adapters_to(
                plan.fragments[0].clone(),
                &mut Vec::new(),
                &mut Timer,
                &RunControl::default(),
                HostedRunAdapters {
                    keyboard: Some(&mut keyboard),
                    indicator: if mismatch == 4 {
                        None
                    } else {
                        Some(&mut provider)
                    },
                },
            )
            .unwrap_err();
        assert!(error.contains("indicator"), "{error}");
        assert_eq!(keyboard.0, 0);
        assert_eq!(provider.calls, 0);
    }
}

#[test]
fn failed_acknowledgment_and_replaced_provider_do_not_become_success() {
    for failure in [
        IndicatorFailure::Lost,
        IndicatorFailure::Timeout,
        IndicatorFailure::MalformedReceipt,
        IndicatorFailure::WrongState,
        IndicatorFailure::Cancelled,
        IndicatorFailure::StaleIdentity,
    ] {
        let (mut host, plan, mut provider) = prepare();
        if failure == IndicatorFailure::StaleIdentity {
            provider.replace_after_first = true;
        } else {
            provider.failure = Some(failure);
        }
        let error = host
            .run_fragment_controlled_with_adapters_to(
                plan.fragments[0].clone(),
                &mut Vec::new(),
                &mut Timer,
                &RunControl::default(),
                HostedRunAdapters {
                    keyboard: Some(&mut Keyboard(0)),
                    indicator: Some(&mut provider),
                },
            )
            .unwrap_err();
        assert!(
            error.contains(&format!("detail: {}", failure as u16)),
            "{error}"
        );
        assert_eq!(provider.calls, 1);
    }
}
