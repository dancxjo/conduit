//! Inert finite pulse provider shared by planning and execution proofs.
use conduit_core::*;
use conduit_pulse_capture::{installation::*, owner::*, *};
use std::{cell::Cell, rc::Rc};

pub struct Provider {
    pub scenario: usize,
    pub effects: Rc<Cell<usize>>,
}
impl PulseReadProvider for Provider {
    async fn read(
        &mut self,
        request: CheckedPulseRead,
    ) -> Result<PulseReadCapture, PulseReadRefusal> {
        assert_eq!(request.request().octets, 5);
        self.effects.set(self.effects.get() + 1);
        let mut wire = if self.scenario == 5 {
            [53, 0, 10, 130, 193]
        } else {
            [56, 0, 19, 8, 83]
        };
        if self.scenario == 1 {
            wire[4] ^= 1;
        }
        Ok(match self.scenario {
            2 => PulseReadCapture::from_complete_octets(PulseReadDisposition::TimedOut, &wire[..2]),
            3 => PulseReadCapture::from_complete_octets(PulseReadDisposition::ProviderFailed, &[]),
            6 => PulseReadCapture::from_complete_octets(PulseReadDisposition::Complete, &wire[..4]),
            _ => PulseReadCapture::from_complete_octets(PulseReadDisposition::Complete, &wire),
        }
        .unwrap())
    }
}
pub fn identity() -> PulseNativeIdentity {
    PulseNativeIdentity {
        host_id: crate::plan::PREFIX.into(),
        boot_id: "fixture/environment/boot".into(),
        base_id: "fixture/pulse-base".into(),
        provider_instance_id: "fixture/pulse-provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pulse-resource".into(),
        resource_generation_id: ResourceGenerationId("fixture/attachment/1".into()),
        envelope_id: "fixture/pulse-envelope".into(),
        artifact_id: "fixture/pulse-artifact".into(),
        implementation_id: "fixture/pulse-read@1".into(),
        base_implementation_id: "fixture/pulse-base@1".into(),
    }
}
pub fn ready(scenario: usize) -> (ReadyPulseBase<Provider>, Rc<Cell<usize>>) {
    ready_with_identity(scenario, identity())
}
pub fn ready_with_identity(
    scenario: usize,
    native: PulseNativeIdentity,
) -> (ReadyPulseBase<Provider>, Rc<Cell<usize>>) {
    let effects = Rc::new(Cell::new(0));
    let attachment = PulseAttachment {
        generation: 1,
        resource_bytes: 64,
        limits: PulseReadLimits {
            maximum_hold_low_us: 50_000,
            maximum_skipped_pulses: 4,
            minimum_sample_offset_us: 10,
            maximum_sample_offset_us: 200,
            maximum_octets: if scenario == 4 { 4 } else { 8 },
            maximum_capture_timeout_us: 50_000,
        },
    };
    // SAFETY: inert fixture owns only this finite transcript, no physical resources.
    let ready = unsafe {
        ReadyPulseBase::new(
            native,
            attachment,
            Provider {
                scenario,
                effects: effects.clone(),
            },
        )
    }
    .unwrap();
    (ready, effects)
}
pub fn grant() -> AuthorityGrant {
    AuthorityGrant {
        grant_id: "fixture/pulse-grant".into(),
        contract_id: PULSE_AUTHORITY.into(),
        host_call_contract_id: PULSE_READ_CALL.into(),
        subject_kind: PULSE_READ_KIND.into(),
        host_id: identity().host_id,
        boot_id: identity().boot_id,
        capability_id: PULSE_CAPABILITY.into(),
    }
}
