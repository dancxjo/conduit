//! One prepared constructor request crossing the normal kernel HostCall.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};
use conduit_speech::ipa_contract::*;
pub(super) static PHONETIC_FACTORY: BackFactory = BackFactory {
    implementation_id: PHONETIC_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static PHONEMIC_FACTORY: BackFactory = BackFactory {
    implementation_id: PHONEMIC_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct IpaAdmissionBack {
    request: ValueRef,
    pending: bool,
    emitted: bool,
}
impl<const PORTS: usize> StepBack<PORTS> for IpaAdmissionBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(0) {
                return failed();
            }
            if outcome.disposition != HostCallDisposition::Completed {
                return failed();
            }
            let Some(output) = outcome.output else {
                return failed();
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_host_completion()
                .expect("observed IPA completion");
            io.send(PortId(0), output.value).expect("ready IPA outcome");
            self.emitted = true;
            self.pending = false;
            StepOutcome::Progress
        } else if !self.pending {
            io.request_host_call(
                RequestId(0),
                HostCallId(0),
                BoundedValueRef::new(self.request, 32).expect("fixed digest"),
            )
            .expect("planned IPA HostCall");
            self.pending = true;
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }
    fn cancel(&mut self) {
        self.emitted = true;
        self.pending = false;
    }
}
fn failed() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::HostCallFailed,
        detail: 88,
    })
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    super::ipa_admission_host::validate(placement)?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: MAXIMUM_IPA_OUTPUT_BYTES + 32,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_IPA_OUTPUT_BYTES,
    })
}
fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let digest = super::ipa_admission_host::request_digest(placement)?;
    let request = values
        .store(&digest)
        .map_err(|error| format!("store IPA request: {error:?}"))?;
    Ok(InstalledBack::IpaAdmission(IpaAdmissionBack {
        request,
        pending: false,
        emitted: false,
    }))
}
