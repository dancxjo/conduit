//! Named atomic-clock fixture; no native hardware or timing claim.
use super::*;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
struct Provider(Arc<AtomicU64>);
impl MonotonicDeadlineProvider for Provider {
    fn poll_until(
        &mut self,
        deadline: u64,
    ) -> Result<Option<u64>, super::super::codec::ClockDisposition> {
        let now = self.0.load(Ordering::SeqCst);
        Ok((now >= deadline).then_some(now))
    }
    fn revoke(&mut self) {}
}
fn native() -> (ClockNativeIdentity, NativeProtocolIssuer) {
    let identity = ClockNativeIdentity {
        host_id: "fixture/control-play-host".into(),
        boot_id: "fixture/control-play-boot".into(),
        base_id: "fixture/controller".into(),
        provider_instance_id: "fixture/owned-bus".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pio".into(),
        resource_generation_id: ResourceGenerationId("fixture/pio-generation".into()),
        envelope_id: "fixture/bus-window".into(),
        artifact_id: "fixture/compiled-control".into(),
    };
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: "fixture/bus-authority".into(),
            contract_id: CLOCK_AUTHORITY.into(),
            host_call_contract_id: CLOCK_CALL.into(),
            subject_kind: CLOCK_KIND.into(),
            host_id: identity.host_id.clone(),
            boot_id: identity.boot_id.clone(),
            capability_id: CLOCK_IMPLEMENTATION.into(),
        },
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: identity.provider_generation,
        resource_pool_id: identity.resource_pool_id.clone(),
        resource_generation_id: identity.resource_generation_id.clone(),
        envelope_id: identity.envelope_id.clone(),
        operation_contract_id: CLOCK_CALL.into(),
        maximum_parameter_bytes: 512,
        maximum_result_bytes: 512,
        maximum_work_units: MAXIMUM_POLL_STEPS,
        maximum_in_flight: 1,
        maximum_operations: 4,
    };
    let issuer = NativeProtocolIssuer::from_test_authority(authority, [8; 32]).unwrap();
    (identity, issuer)
}
#[test]
fn clock_play_reuses_normal_admitted_calls_and_waits_for_actual_observation_without_allocating() {
    let (identity, issuer) = native();
    let now = Arc::new(AtomicU64::new(0));
    let ready = unsafe { ReadyClockBase::new(identity.clone(), Provider(now.clone())) }.unwrap();
    let mut play = ClockPlay::prepare(ready, &identity, issuer).unwrap();
    let allocations = crate::test_allocations::allocations(|| {
        play.start().unwrap();
        for deadline in [20, 40] {
            play.submit(deadline).unwrap();
            for _ in 0..16 {
                assert!(!play.step().unwrap());
            }
            assert!(play.pending());
            now.store(deadline, Ordering::SeqCst);
            let mut complete = false;
            for _ in 0..32 {
                if play.step().unwrap() {
                    complete = true;
                    break;
                }
            }
            assert!(complete);
        }
    });
    assert_eq!(allocations, 0);
}
