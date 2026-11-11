//! A prepared startup pulse, bound only after the owner's durable acknowledgment.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef, ValueStorage,
};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
#[derive(Clone)]
pub(super) struct StartupGate(Arc<AtomicU8>);
impl StartupGate {
    pub(super) fn bind(&self, eligible: bool) -> Result<(), String> {
        self.0
            .compare_exchange(
                0,
                if eligible { 1 } else { 2 },
                Ordering::Release,
                Ordering::Relaxed,
            )
            .map(|_| ())
            .map_err(|_| "startup input was already bound".into())
    }
}
pub(super) struct StartupSourceBack {
    pub(super) gate: StartupGate,
    pulse: Option<ValueRef>,
}
impl<const PORTS: usize> StepBack<PORTS> for StartupSourceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self.gate.0.load(Ordering::Acquire) {
            0 => {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 471,
                })
            }
            2 => {
                self.pulse = None;
                return StepOutcome::Complete;
            }
            _ => {}
        }
        let Some(pulse) = self.pulse else {
            return StepOutcome::Complete;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.send(PortId(0), pulse).is_err() {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidLifecycle,
                detail: 472,
            });
        }
        self.pulse = None;
        StepOutcome::Complete
    }
    fn cancel(&mut self) {
        self.pulse = None;
    }
}
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::BODY_WAKE_IMPLEMENTATION,
    budget,
    prepare,
};
fn validate(p: &PlannedGear) -> Result<(), String> {
    let o = conduit_std_offers::body_wake_offer();
    if p.kind_id != o.kind_id
        || p.kind_contract_revision != o.kind_contract_revision
        || p.semantic_contract != o.semantic_contract
        || p.execution_profile_id != o.implementation.execution_profile_id
        || p.implementation_id != o.implementation.implementation_id
        || p.artifact_id != o.implementation.artifact_id
        || p.inputs != o.inputs
        || p.outputs != o.outputs
        || p.host_calls != o.host_calls
        || p.limits != o.limits
        || !p.configuration.is_empty()
        || !p.authority.is_empty()
        || !p.resources.is_empty()
        || !p.terminal_transductions.is_empty()
    {
        return Err("body/wake differs from installed causal startup source".into());
    }
    Ok(())
}
fn budget(p: &PlannedGear) -> Result<BackBudget, String> {
    validate(p)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
    })
}
fn prepare(
    p: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(p)?;
    let pulse = values
        .store(&conduit_core::InfoBool::new(true).encode())
        .map_err(|e| format!("startup pulse storage: {e:?}"))?;
    Ok(InstalledBack::StartupSource(StartupSourceBack {
        gate: StartupGate(Arc::new(AtomicU8::new(0))),
        pulse: Some(pulse),
    }))
}
