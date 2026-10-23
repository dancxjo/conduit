//! Production Root issuance with inert CPU providers; no physical IO or Body.
use super::*;
use conduit_monotonic_wait as w;
use conduit_pulse_capture::*;
mod plan {
    pub const PREFIX: &str = "host/network";
}
#[path = "../../../runtime/tests/support/assignment.rs"]
#[allow(dead_code)] // Dependency of the shared provider's unused binding helpers.
mod assignment;
#[path = "../../../runtime/tests/support/pulse.rs"]
#[allow(dead_code)] // Reuse only the retained CPU transcript provider.
mod pulse_fixture;
struct Timer;
impl w::MonotonicWaitProvider for Timer {
    async fn wait(&mut self, _: u64) -> Result<(), w::WaitProviderFailure> {
        panic!("issuance must not dispatch a timer")
    }
}

#[test]
fn pulse_and_wait_have_independent_finite_root_authority_for_the_same_source() {
    let source = conduit_environment_make::compile_appliance_source().unwrap();
    let cadence = source.checked_cadence().unwrap();
    let mut issuer = native_authority::RootIssuer::observe(&boot::ObservedBoot).unwrap();
    let mut native = pulse_fixture::identity();
    native.boot_id = "boot/current".into();
    let (pulse, effects) = pulse_fixture::ready_with_identity(0, native.clone());
    assert_eq!(pulse.native_identity(), &native);
    let timer_identity = w::WaitNativeIdentity {
        host_id: native.host_id.clone(),
        boot_id: native.boot_id.clone(),
        base_id: "fixture/timer-base".into(),
        provider_instance_id: "fixture/timer".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/timer-resource".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/timer-generation".into()),
        envelope_id: "fixture/timer-envelope".into(),
        artifact_id: "fixture/timer-artifact".into(),
        implementation_id: "fixture/timer@1".into(),
        base_implementation_id: "fixture/timer-base@1".into(),
    };
    // SAFETY: exclusively owned inert CPU timer fixture with one finite slot.
    let timer = unsafe {
        w::ReadyMonotonicWaitBase::new(
            timer_identity.clone(),
            w::WaitAttachment {
                generation: 1,
                resource_bytes: 32,
                maximum_duration_ms: 60000,
            },
            Timer,
        )
    }
    .unwrap();
    let mut software =
        conduit_environment_runtime::appliance::software::ReadySoftwareBacks::prepare(
            &source,
            native.host_id.clone(),
            native.boot_id.clone(),
        )
        .unwrap();
    let mut host = fixtures::host();
    pulse
        .append_to_advertisement(&mut host, &PulseReadContract::prepare().unwrap())
        .unwrap();
    timer.append_to_advertisement(&mut host).unwrap();
    software.append_to_advertisement(&mut host).unwrap();
    host.bases.sort_by(|a, b| a.base_id.cmp(&b.base_id));
    host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let grants = [
        issuer.grant_pulse(&native).unwrap(),
        issuer.grant_wait(&timer_identity).unwrap(),
    ];
    let grants = super::policy::round_trip(&source, &host, grants);
    let selected = source
        .plan_local(&host, &["conduit.base/local@1".into()], &grants)
        .unwrap();
    let fragment = &selected.fragments[0];
    let tick = fragment
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == conduit_time::TICK_KIND)
        .unwrap();
    let tick_cord = fragment
        .connections
        .iter()
        .find(|cord| cord.source_placement_id == tick.placement_id)
        .unwrap();
    assert_eq!(tick_cord.byte_capacity, 64);
    let mut undersized = host.clone();
    undersized
        .capabilities
        .iter_mut()
        .find(|offer| {
            offer.kind_id.as_str()
                == conduit_environment_runtime::publication::PublicationChannel::Reading.kind_id()
        })
        .unwrap()
        .limits
        .max_queue_bytes = 64;
    assert!(matches!(
        source.plan_local(&undersized, &["conduit.base/local@1".into()], &grants),
        Err(conduit_planner::PlannerError::QueueRequirementAboveHostLimit(_))
    ));

    let unreserved = c::bind_active_play(&fragment.plan_id, &native.host_id, &native.boot_id, 1);
    assert!(issuer
        .issue_pulse(&native, fragment, &unreserved, &cadence)
        .is_err());
    let active = issuer.reserve_play(fragment).unwrap();
    for mode in 0..5 {
        let mut wrong = native.clone();
        match mode {
            0 => wrong.boot_id = "stale/boot".into(),
            1 => wrong.provider_generation += 1,
            2 => wrong.artifact_id = "foreign/artifact".into(),
            3 => wrong.resource_pool_id = "foreign/resource".into(),
            _ => wrong.base_implementation_id = "foreign/base@1".into(),
        }
        assert!(issuer
            .issue_pulse(&wrong, fragment, &active, &cadence)
            .is_err());
    }
    for wrong in [
        conduit_time::TickConfiguration {
            count: cadence.count + 1,
            ..cadence.clone()
        },
        conduit_time::TickConfiguration {
            period_ms: cadence.period_ms + 1,
            ..cadence.clone()
        },
        conduit_time::TickConfiguration {
            count: 0,
            ..cadence.clone()
        },
    ] {
        assert!(issuer
            .issue_pulse(&native, fragment, &active, &wrong)
            .is_err());
    }
    let (mut pulse_table, mut pulse_handle, pulse_claim) = issuer
        .issue_pulse(&native, fragment, &active, &cadence)
        .unwrap();
    let (mut wait_table, mut wait_handle, wait_claim) = issuer
        .issue_wait(&timer_identity, fragment, &active, &cadence)
        .unwrap();
    let scope = &pulse_table.inspections().next().unwrap().scope;
    assert_eq!(scope.maximum_operations, cadence.count as u32);
    assert_eq!(scope.maximum_parameter_bytes, PULSE_READ_MAXIMUM_BYTES);
    assert_eq!(scope.maximum_result_bytes, PULSE_READ_MAXIMUM_BYTES);
    assert_eq!(scope.maximum_in_flight, 1);
    assert_eq!(scope.maximum_work_units, 1);
    assert_eq!(scope.subject_kind.as_str(), PULSE_READ_KIND);
    assert_ne!(pulse_claim.base_instance_id, wait_claim.base_instance_id);
    let (_, memory) = allocation_probe::observe(|| {
        assert!(pulse_table.authorize(&pulse_handle, &wait_claim).is_err());
        assert!(wait_table.authorize(&wait_handle, &pulse_claim).is_err());
        for _ in 0..cadence.count {
            let lease = pulse_table.authorize(&pulse_handle, &pulse_claim).unwrap();
            pulse_table.complete(&mut pulse_handle, lease, 0).unwrap();
            let lease = wait_table.authorize(&wait_handle, &wait_claim).unwrap();
            wait_table.complete(&mut wait_handle, lease, 0).unwrap();
        }
        assert!(pulse_table.authorize(&pulse_handle, &pulse_claim).is_err());
        assert!(wait_table.authorize(&wait_handle, &wait_claim).is_err());
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert!(issuer
        .issue_pulse(&native, fragment, &active, &cadence)
        .is_err());
    assert_eq!(effects.get(), 0);
}
