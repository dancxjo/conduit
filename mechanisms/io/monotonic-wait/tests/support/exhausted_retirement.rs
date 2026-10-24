//! A separately checked one-tick Source uses a genuinely finite one-call budget.
use super::*;
#[test]
fn exhausted_timer_refuses_retirement_until_explicit_revocation() {
    let state = Rc::new(State::default());
    let (mut fragment, _, _, placement) = fixture::selected();
    let configuration = vec![
        ConfigurationEntry {
            key: "count".into(),
            value: ConfigurationValue::U64(1),
        },
        ConfigurationEntry {
            key: "period-ms".into(),
            value: ConfigurationValue::U64(250),
        },
    ];
    fragment.placements[0].configuration = configuration.clone();
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = seal_plan(identity, vec![fragment]).fragments.remove(0);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    let mut scope = fixture::scope();
    scope.maximum_operations = 1;
    let (table, handle, claim) = fixture::authorization(scope, &fragment, &active);
    let attachment = WaitAttachment {
        generation: 7,
        resource_bytes: 4096,
        maximum_duration_ms: 1000,
    };
    // SAFETY: fixture retains the inert provider and independently configured one-tick Source.
    let owner = unsafe {
        MonotonicWaitHostCall::bind_selected(
            table,
            handle,
            claim,
            attachment,
            Provider(state.clone()),
            &configuration,
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement,
            },
        )
    }
    .unwrap();
    let node = lowered.nodes[0].node;
    let mut retained = Some(owner);
    let input = encode_monotonic_duration(250);
    let (_, measured) = allocation_probe::observe(|| {
        assert_eq!(
            immediate(
                retained
                    .as_mut()
                    .unwrap()
                    .invoke(node, HostCallId(0), RequestId(0), &input)
            ),
            Ok(())
        );
        let owner = retained.as_ref().unwrap();
        assert_eq!(owner.inspection().lifecycle, CapabilityLifecycle::Exhausted);
        assert_eq!(owner.inspection().completed_operations, 1);
        assert_eq!(owner.inspection().in_flight_operations, 0);
        let address = owner as *const _;
        assert!(matches!(
            MonotonicWaitHostCall::retire_retained(&mut retained),
            Err(WaitRetirementRefusal::NotRevoked)
        ));
        assert_eq!(retained.as_ref().unwrap() as *const _, address);
        assert_eq!(
            retained.as_ref().unwrap().inspection().lifecycle,
            CapabilityLifecycle::Exhausted
        );
        assert!(immediate(retained.as_mut().unwrap().invoke(
            node,
            HostCallId(0),
            RequestId(1),
            &input
        ))
        .is_err());
        assert_eq!(state.calls.get(), 1);
        retained
            .as_mut()
            .unwrap()
            .revoke(node, HostCallId(0))
            .unwrap();
        let (provider, recovered) = MonotonicWaitHostCall::retire_retained(&mut retained).unwrap();
        assert!(Rc::ptr_eq(&provider.0, &state));
        assert_eq!(recovered, attachment);
        assert!(retained.is_none());
        assert!(matches!(
            MonotonicWaitHostCall::retire_retained(&mut retained),
            Err(WaitRetirementRefusal::Unavailable)
        ));
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
