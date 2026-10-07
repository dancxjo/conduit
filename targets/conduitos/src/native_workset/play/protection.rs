//! Hardware domains beneath the existing Body scheduler and Host Call boundary.
use super::*;
use crate::{composition::MachineRunError, protection_domain::KernelRevocationCause};

impl NativeWorksetPlay {
    pub(super) fn activate_protection(
        &mut self,
        plan: &conduit_body::BodyPlan,
        play: &conduit_body::BodyPlayIdentity,
    ) -> Result<(), PlayRefusal> {
        for plot in 0..self.plot_count {
            if let Some(admission) = self.protection_admissions[plot].take() {
                self.protected[plot] = Some(
                    admission
                        .activate(plan, play)
                        .map_err(PlayRefusal::Protection)?,
                );
            }
        }
        Ok(())
    }

    pub(super) fn revoke_protection(&mut self, cause: KernelRevocationCause) {
        for domain in self.protected.iter_mut().flatten() {
            domain.revoke(cause);
        }
        self.protection_admissions.fill_with(|| None);
    }

    pub(super) fn protected_failure(
        &mut self,
        request: HostCallRequest,
        error: MachineRunError,
    ) -> Result<(), PlayRefusal> {
        use conduit_kernel::FailureCode;
        let code = match error {
            MachineRunError::ProtectionFault(
                crate::protected_region::DomainFault::WorkExhausted,
            ) => FailureCode::WorkBudgetExhausted,
            MachineRunError::TextMalformedUtf8 => FailureCode::InvalidInput,
            MachineRunError::TextOutputOverflow => FailureCode::StorageExhausted,
            _ => FailureCode::HostCallFailed,
        };
        self.complete_failure(request, code, 84)?;
        self.revoke_protection(KernelRevocationCause::PlayFailed);
        Err(PlayRefusal::Protection(error))
    }
}
