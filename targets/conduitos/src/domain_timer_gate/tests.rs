use super::*;
use crate::{identity::BootIdentities, offer::CpuFeatures, protection_domain::ProtectionDomainId};
use conduit_kernel::{BoundedValueRef, RequestId, ValueRef};

fn fixture(domain: u32) -> TimerGate {
    let ids = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let fixed = HostOffer::new(
        &ids,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    let prepared = crate::tour_timer_plan::prepare(&ids, &fixed, "build").unwrap();
    let binding = RegionBinding::admit(
        &prepared.plan,
        &prepared.active_play,
        &prepared.plan.fragments[0].execution_regions[0].region_id,
        ProtectionDomainId(domain),
    )
    .unwrap();
    TimerGate::admit(&prepared.plan, binding, &fixed, 31).unwrap()
}

// Synthetic bounded requests isolate Root gate correlation from runtime proof.
fn request(node: NodeId, sequence: u32) -> HostCallRequest {
    HostCallRequest {
        node,
        request: RequestId(sequence),
        call: HostCallId(0),
        input: BoundedValueRef {
            value: ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 8,
            },
            admitted_bytes: 8,
        },
    }
}

#[test]
fn tour_timer_gate_keeps_async_lease_until_exact_current_wake_without_allocating() {
    let mut gate = fixture(4);
    let timer = request(gate.timer_node, 1);
    let count = request(gate.count_node, 1);
    let (timer_handle, count_handle) = gate.handles();
    let allocations = crate::test_allocations::allocations(|| {
        gate.begin_wait(
            timer,
            timer_handle,
            &120_u64.to_le_bytes(),
            1,
            Some(1),
            Some(1),
        )
        .unwrap();
        assert_eq!(gate.pending_wait(), Some(interest(timer)));
        assert_eq!(
            gate.begin_wait(
                request(gate.timer_node, 2),
                timer_handle,
                &120_u64.to_le_bytes(),
                1,
                Some(1),
                Some(1)
            ),
            Err(TimerGateRefusal::Request)
        );
        // Count effects remain independent while the timer lease is in flight.
        gate.begin_count(count, count_handle, b"18446744073709551615", 1, Some(1))
            .unwrap();
        let mut wrong = count;
        wrong.input.value.generation += 1;
        assert_eq!(
            gate.complete_count(wrong, Some(1)),
            Err(TimerGateRefusal::Request)
        );
        gate.complete_count(count, Some(1)).unwrap();
        let count = request(gate.count_node, 2);
        gate.begin_count(count, count_handle, b"7", 1, Some(1))
            .unwrap();
        gate.complete_count(count, Some(1)).unwrap();
        let mut wrong = interest(timer);
        wrong.request.0 += 1;
        assert_eq!(
            gate.complete_wait(wrong, Some(1), Some(1)),
            Err(TimerGateRefusal::Request)
        );
        assert!(matches!(
            gate.complete_wait(interest(timer), Some(1), Some(2)),
            Err(TimerGateRefusal::Binding(_))
        ));
        assert_eq!(gate.pending_wait(), Some(interest(timer)));
        gate.complete_wait(interest(timer), Some(1), Some(1))
            .unwrap();
        assert_eq!(
            gate.complete_wait(interest(timer), Some(1), Some(1)),
            Err(TimerGateRefusal::Request)
        );
        gate.begin_wait(
            request(gate.timer_node, 2),
            timer_handle,
            &120_u64.to_le_bytes(),
            1,
            Some(1),
            Some(1),
        )
        .unwrap();
        let revoked = gate.revoke(KernelRevocationCause::PlayCancelled);
        assert_eq!(revoked.revoked_handles, 2);
        assert_eq!(gate.pending_wait(), None);
        assert_eq!(
            gate.complete_wait(interest(timer), Some(1), Some(1)),
            Err(TimerGateRefusal::Revoked)
        );
        assert_eq!(
            gate.begin_count(count, count_handle, b"7", 1, Some(1)),
            Err(TimerGateRefusal::Revoked)
        );
    });
    assert_eq!(allocations, 0);
}

#[test]
fn tour_timer_gate_refuses_cross_used_and_foreign_handles_before_effect() {
    let mut gate = fixture(4);
    let foreign = fixture(5).handles().0;
    let timer = request(gate.timer_node, 1);
    let count = request(gate.count_node, 1);
    let (timer_handle, count_handle) = gate.handles();
    assert_eq!(
        gate.begin_wait(
            timer,
            count_handle,
            &120_u64.to_le_bytes(),
            1,
            Some(1),
            Some(1)
        ),
        Err(TimerGateRefusal::Capability(
            KernelCapabilityRefusal::WrongScope
        ))
    );
    assert_eq!(
        gate.begin_count(count, timer_handle, b"0", 1, Some(1)),
        Err(TimerGateRefusal::Capability(
            KernelCapabilityRefusal::WrongScope
        ))
    );
    assert_eq!(
        gate.begin_wait(timer, foreign, &120_u64.to_le_bytes(), 1, Some(1), Some(1)),
        Err(TimerGateRefusal::Capability(
            KernelCapabilityRefusal::UnknownHandle
        ))
    );
    assert!(gate.pending_wait().is_none());
    gate.begin_wait(
        timer,
        timer_handle,
        &120_u64.to_le_bytes(),
        1,
        Some(1),
        Some(1),
    )
    .unwrap();
    gate.begin_count(count, count_handle, b"0", 1, Some(1))
        .unwrap();
}

#[test]
fn tour_timer_gate_refuses_broadened_parameters_work_epochs_and_digit_windows() {
    let mut gate = fixture(4);
    let timer = request(gate.timer_node, 1);
    let count = request(gate.count_node, 1);
    let (timer_handle, count_handle) = gate.handles();
    for duration in [0_u64, 119, 121, u64::MAX] {
        assert_eq!(
            gate.begin_wait(
                timer,
                timer_handle,
                &duration.to_le_bytes(),
                1,
                Some(1),
                Some(1)
            ),
            Err(TimerGateRefusal::Request)
        );
    }
    for work in [0, 2, u32::MAX] {
        assert_eq!(
            gate.begin_wait(
                timer,
                timer_handle,
                &120_u64.to_le_bytes(),
                work,
                Some(1),
                Some(1)
            ),
            Err(TimerGateRefusal::Request)
        );
    }
    for epochs in [
        (None, Some(1)),
        (Some(1), None),
        (Some(2), Some(1)),
        (Some(1), Some(2)),
    ] {
        assert!(matches!(
            gate.begin_wait(
                timer,
                timer_handle,
                &120_u64.to_le_bytes(),
                1,
                epochs.0,
                epochs.1
            ),
            Err(TimerGateRefusal::Binding(_))
        ));
    }
    for digits in [b"".as_slice(), b"00", b"-1", b"a", b"123456789012345678901"] {
        assert_eq!(
            gate.begin_count(count, count_handle, digits, 1, Some(1)),
            Err(TimerGateRefusal::Request)
        );
    }
    assert!(matches!(
        gate.begin_count(count, count_handle, b"0", 1, Some(2)),
        Err(TimerGateRefusal::Binding(_))
    ));
    let mut wrong = timer;
    wrong.node = gate.count_node;
    assert_eq!(
        gate.begin_wait(
            wrong,
            timer_handle,
            &120_u64.to_le_bytes(),
            1,
            Some(1),
            Some(1)
        ),
        Err(TimerGateRefusal::Request)
    );
    gate.begin_wait(
        timer,
        timer_handle,
        &120_u64.to_le_bytes(),
        1,
        Some(1),
        Some(1),
    )
    .unwrap();
    gate.begin_count(count, count_handle, b"0", 1, Some(1))
        .unwrap();
}
