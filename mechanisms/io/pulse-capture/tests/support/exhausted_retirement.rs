//! Exhaustion is not revocation; actual provider custody survives refusal.
use super::*;
#[test]
fn exhausted_pulse_refuses_retirement_until_explicit_revocation() {
    let state = Rc::new(State::default());
    let (fragment, lowered, active, placement) = fixture::selected();
    let owner = fixture::bind_with_budget(
        &fragment,
        &lowered,
        &active,
        &placement,
        Provider(state.clone()),
        1,
    )
    .unwrap();
    let node = lowered.nodes[0].node;
    let mut retained = Some(owner);
    let input = request(5);
    let (_, measured) = allocation_probe::observe(|| {
        assert!(is_tag(
            ready(
                retained
                    .as_mut()
                    .unwrap()
                    .invoke(node, HostCallId(0), RequestId(0), &input)
            )
            .unwrap(),
            "completed"
        ));
        let owner = retained.as_ref().unwrap();
        assert_eq!(owner.inspection().lifecycle, CapabilityLifecycle::Exhausted);
        assert_eq!(owner.inspection().completed_operations, 1);
        assert_eq!(owner.inspection().in_flight_operations, 0);
        let address = owner as *const _;
        assert!(matches!(
            PulseHostCall::retire_retained(&mut retained),
            Err(PulseRetirementRefusal::NotRevoked)
        ));
        assert_eq!(retained.as_ref().unwrap() as *const _, address);
        assert_eq!(
            retained.as_ref().unwrap().inspection().lifecycle,
            CapabilityLifecycle::Exhausted
        );
        assert!(ready(retained.as_mut().unwrap().invoke(
            node,
            HostCallId(0),
            RequestId(1),
            &input
        ))
        .is_err());
        assert_eq!(state.effects.get(), 1);
        retained
            .as_mut()
            .unwrap()
            .revoke(node, HostCallId(0))
            .unwrap();
        let (provider, attachment) = PulseHostCall::retire_retained(&mut retained).unwrap();
        assert!(Rc::ptr_eq(&provider.0, &state));
        assert_eq!(attachment.generation, 7);
        assert_eq!(attachment.resource_bytes, 4096);
        assert_eq!(attachment.limits, limits());
        assert!(retained.is_none());
        assert!(matches!(
            PulseHostCall::retire_retained(&mut retained),
            Err(PulseRetirementRefusal::Unavailable)
        ));
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
