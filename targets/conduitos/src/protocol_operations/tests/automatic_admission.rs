//! Fixture-owned authority exercises the production native issuer.
use super::*;
use crate::cryptographic_entropy::{
    CryptographicEntropyBase, CryptographicEntropySource, EntropyProvider, EntropyRefusal,
};
use crate::protocol_source::{NativeProtocolIssueRefusal, NativeProtocolIssuer};

enum FixtureEntropy {
    Strong,
    Weak,
    Unavailable,
}
impl CryptographicEntropySource for FixtureEntropy {
    fn provider(&self) -> EntropyProvider {
        EntropyProvider {
            base_id: "fixture/entropy",
            provider_instance_id: "fixture/entropy-provider",
            provider_generation: 1,
        }
    }
    fn fill_exact(&mut self, output: &mut [u8]) -> Result<(), EntropyRefusal> {
        match self {
            Self::Strong => {
                output.fill(7);
                Ok(())
            }
            Self::Weak => {
                output.fill(0);
                Ok(())
            }
            Self::Unavailable => Err(EntropyRefusal::SourceFailure),
        }
    }
}

fn issuer(authority: BaseCapabilityAuthority) -> NativeProtocolIssuer {
    let mut entropy = CryptographicEntropyBase::<_, 1>::admit(FixtureEntropy::Strong).unwrap();
    // SAFETY: this test Root owns the independently declared fixture authority;
    // all providers and entropy here are explicitly fixtures with no hardware effects.
    unsafe { NativeProtocolIssuer::admit(authority, &mut entropy) }.unwrap()
}

fn authority(clock: bool, grant: &AuthorityGrant) -> BaseCapabilityAuthority {
    let (name, _, operation, work, bytes) = if clock {
        (
            "clock",
            crate::monotonic_clock::installation::CLOCK_IMPLEMENTATION,
            crate::monotonic_clock::contract::CLOCK_CALL,
            crate::monotonic_clock::owner::MAXIMUM_POLL_STEPS,
            crate::monotonic_clock::contract::CLOCK_MAXIMUM_BYTES,
        )
    } else {
        (
            "bus",
            crate::i2c_base::installation::I2C_IMPLEMENTATION,
            crate::i2c_base::contract::I2C_CALL,
            1,
            crate::i2c_base::contract::I2C_MAXIMUM_BYTES,
        )
    };
    // The fixture Root supplied this grant before planning, independently of
    // Source and the resulting selected placement.
    BaseCapabilityAuthority {
        grant: grant.clone(),
        base_instance_id: alloc::format!("fixture/{name}-provider").into(),
        base_provider_generation: 1,
        resource_pool_id: alloc::format!("fixture/{name}-resource").into(),
        resource_generation_id: ResourceGenerationId(alloc::format!("fixture/{name}-generation")),
        operation_contract_id: operation.into(),
        envelope_id: alloc::format!("fixture/{name}-envelope").into(),
        maximum_parameter_bytes: bytes,
        maximum_result_bytes: bytes,
        maximum_work_units: work,
        maximum_in_flight: 1,
        maximum_operations: 256,
    }
}

pub(super) fn native_issuer(clock: bool, grant: &AuthorityGrant) -> NativeProtocolIssuer {
    issuer(authority(clock, grant))
}

pub(super) fn possession(
    plan: &Plan,
    clock: bool,
    grant: &AuthorityGrant,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
    let fragment = &plan.fragments[0];
    let authority = authority(clock, grant);
    let work = authority.maximum_work_units;
    let implementation = if clock {
        crate::monotonic_clock::installation::CLOCK_IMPLEMENTATION
    } else {
        crate::i2c_base::installation::I2C_IMPLEMENTATION
    };
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let implementation = ImplementationId::from(implementation);
    for source in [FixtureEntropy::Weak, FixtureEntropy::Unavailable] {
        let mut entropy = CryptographicEntropyBase::<_, 1>::admit(source).unwrap();
        // SAFETY: fixture Root authority and fixture entropy; no hardware access.
        let result = unsafe { NativeProtocolIssuer::admit(authority.clone(), &mut entropy) };
        assert!(matches!(
            result,
            Err(NativeProtocolIssueRefusal::Entropy(_))
        ));
        assert_eq!(entropy.requests_used(), 1);
    }
    for altered in 0..4 {
        let mut wrong = authority.clone();
        match altered {
            0 => wrong.grant.grant_id = "fixture/forged-grant".into(),
            1 => wrong.base_provider_generation += 1,
            2 => wrong.resource_pool_id = "fixture/another-resource".into(),
            _ => wrong.operation_contract_id = "fixture/another-operation".into(),
        }
        assert!(matches!(
            issuer(wrong).issue(plan, &active, &implementation, work),
            Err(NativeProtocolIssueRefusal::WrongSelection)
        ));
    }
    let mut forged_play = active.clone();
    forged_play.active_play_id = "fixture/forged-play".into();
    assert!(matches!(
        issuer(authority.clone()).issue(plan, &forged_play, &implementation, work),
        Err(NativeProtocolIssueRefusal::WrongSelection)
    ));
    let mut insufficient = authority.clone();
    insufficient.maximum_work_units = 0;
    assert!(matches!(
        issuer(insufficient).issue(plan, &active, &implementation, work),
        Err(NativeProtocolIssueRefusal::Capability(_))
    ));
    let possession = issuer(authority)
        .issue(plan, &active, &implementation, work)
        .unwrap();
    (possession.table, possession.handle, possession.claim)
}
