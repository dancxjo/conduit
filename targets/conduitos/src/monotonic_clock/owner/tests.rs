use super::*;
mod fixture;

#[derive(Default)]
struct Provider {
    calls: u64,
    next: Option<u64>,
    revoked: bool,
}
impl MonotonicDeadlineProvider for Provider {
    fn poll_until(&mut self, _: u64) -> Result<Option<u64>, ClockDisposition> {
        assert!(!self.revoked);
        self.calls += 1;
        Ok(self.next.take())
    }
    fn revoke(&mut self) {
        self.revoked = true;
    }
    fn sample_now(&mut self) -> Result<u64, ClockDisposition> {
        self.next.take().ok_or(ClockDisposition::Unavailable)
    }
}
#[test]
fn selected_observation_carries_exact_clock_identity_and_refuses_stale_reads() {
    let mut owner = fixture::owner();
    owner.provider.next = Some(12);
    let observed = owner.observe_monotonic(NodeId(0), HostCallId(0)).unwrap();
    assert_eq!(observed.ticks(), 12);
    assert_eq!(observed.clock().host_id().as_str(), "host/one");
    assert_eq!(observed.clock().boot_id().as_str(), "boot/one");
    assert_eq!(
        observed.clock().basis_id(),
        "base/clock/instance/4/resource-generation/7"
    );
    assert_eq!(observed.clock().scale(), TemporalScale::Milliseconds);
    owner
        .start(NodeId(0), HostCallId(0), RequestId(0), &request(20))
        .unwrap();
    assert_eq!(
        owner.observe_monotonic(NodeId(0), HostCallId(0)),
        Err(ClockCallRefusal::Pending)
    );
    owner.provider.next = Some(20);
    owner.poll(NodeId(0), HostCallId(0), RequestId(0)).unwrap();
    owner.provider.next = Some(11);
    assert_eq!(
        owner.observe_monotonic(NodeId(0), HostCallId(0)),
        Err(ClockCallRefusal::Provider(ClockDisposition::ProviderLost))
    );
    assert_eq!(
        owner.observe_monotonic(NodeId(1), HostCallId(0)),
        Err(ClockCallRefusal::WrongBinding)
    );
    owner.revoke(NodeId(0), HostCallId(0)).unwrap();
    assert_eq!(
        owner.observe_monotonic(NodeId(0), HostCallId(0)),
        Err(ClockCallRefusal::Cancelled)
    );
}
fn request(deadline: u64) -> alloc::vec::Vec<u8> {
    let contract = MonotonicClockContract::prepare().unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!("clock request");
    };
    StructuredInfoValue::record(
        contract.request_type().clone(),
        alloc::vec![
            StructuredFieldValue::new(
                "deadline",
                StructuredInfoValue::leaf(
                    fields[0].value_type().clone(),
                    deadline.to_le_bytes().to_vec(),
                )
                .unwrap()
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn tag(bytes: &[u8]) -> alloc::string::String {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        panic!("clock result");
    };
    tag.into()
}
#[test]
fn deadline_completion_uses_observed_time_and_retains_one_pending_lease() {
    let mut owner = fixture::owner();
    owner
        .start(NodeId(0), HostCallId(0), RequestId(0), &request(10))
        .unwrap();
    assert!(
        owner
            .poll(NodeId(0), HostCallId(0), RequestId(0))
            .unwrap()
            .is_none()
    );
    assert_eq!(owner.provider.calls, 1);
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        1
    );
    owner.provider.next = Some(12);
    let bytes = owner
        .poll(NodeId(0), HostCallId(0), RequestId(0))
        .unwrap()
        .unwrap();
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("completion");
    };
    assert_eq!(tag, "completed");
    assert!(
        matches!(payload.shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == 12_u64.to_le_bytes())
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    assert_eq!(
        owner.start(NodeId(0), HostCallId(0), RequestId(0), &request(10)),
        Err(ClockCallRefusal::StaleRequest)
    );
}
#[test]
fn early_or_regressing_provider_time_is_malformed_without_a_retry() {
    let mut owner = fixture::owner();
    owner
        .start(NodeId(0), HostCallId(0), RequestId(0), &request(10))
        .unwrap();
    owner.provider.next = Some(9);
    assert_eq!(
        tag(owner
            .poll(NodeId(0), HostCallId(0), RequestId(0))
            .unwrap()
            .unwrap()),
        "malformed"
    );
    owner
        .start(NodeId(0), HostCallId(0), RequestId(1), &request(10))
        .unwrap();
    owner.provider.next = Some(20);
    assert_eq!(
        tag(owner
            .poll(NodeId(0), HostCallId(0), RequestId(1))
            .unwrap()
            .unwrap()),
        "completed"
    );
    owner
        .start(NodeId(0), HostCallId(0), RequestId(2), &request(0))
        .unwrap();
    owner.provider.next = Some(19);
    assert_eq!(
        tag(owner
            .poll(NodeId(0), HostCallId(0), RequestId(2))
            .unwrap()
            .unwrap()),
        "malformed"
    );
    assert_eq!(owner.provider.calls, 3);
}
#[test]
fn finite_poll_budget_and_revocation_prevent_unbounded_or_late_completion() {
    let mut owner = fixture::owner();
    owner
        .start(NodeId(0), HostCallId(0), RequestId(0), &request(10))
        .unwrap();
    for _ in 0..MAXIMUM_POLL_STEPS {
        assert!(
            owner
                .poll(NodeId(0), HostCallId(0), RequestId(0))
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(
        tag(owner
            .poll(NodeId(0), HostCallId(0), RequestId(0))
            .unwrap()
            .unwrap()),
        "timeout"
    );
    assert_eq!(owner.provider.calls, MAXIMUM_POLL_STEPS);
    owner
        .start(NodeId(0), HostCallId(0), RequestId(1), &request(10))
        .unwrap();
    owner.revoke(NodeId(0), HostCallId(0)).unwrap();
    owner.provider.next = Some(20);
    assert_eq!(
        owner.poll(NodeId(0), HostCallId(0), RequestId(1)),
        Err(ClockCallRefusal::Cancelled)
    );
    assert!(owner.provider.revoked);
    assert_eq!(owner.provider.calls, MAXIMUM_POLL_STEPS);
}

#[test]
fn substituted_lowering_stale_play_and_invalid_inputs_never_observe_the_provider() {
    let (fragment, mut lowered, active, placement) = fixture::selected();
    lowered.nodes[0].inputs[0].port = conduit_kernel::PortId(1);
    assert!(matches!(
        fixture::bind(&fragment, &lowered, &active, &placement),
        Err(ClockCallRefusal::WrongBinding)
    ));
    let (fragment, lowered, mut active, placement) = fixture::selected();
    active.play_sequence += 1;
    assert!(matches!(
        fixture::bind(&fragment, &lowered, &active, &placement),
        Err(ClockCallRefusal::WrongBinding)
    ));
    let mut owner = fixture::owner();
    assert_eq!(
        owner.start(NodeId(1), HostCallId(0), RequestId(0), &request(0)),
        Err(ClockCallRefusal::WrongBinding)
    );
    assert!(matches!(
        owner.start(NodeId(0), HostCallId(0), RequestId(0), &[]),
        Err(ClockCallRefusal::Canonical(_))
    ));
    assert_eq!(owner.provider.calls, 0);
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    owner
        .start(NodeId(0), HostCallId(0), RequestId(0), &request(0))
        .unwrap();
}
