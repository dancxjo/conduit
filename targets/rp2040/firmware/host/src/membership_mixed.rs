//! Native receiver composition using actual Root owners and the shared loop.
use super::*;
use crate::{mixed_workset as m, resident_preparation as resident};
use core::cell::RefCell;
use embassy_time::Instant;
impl MembershipState {
    pub(super) async fn run_mixed(
        &mut self,
        link: &mut Link,
        workspace: &conduit_body::HostCommandInput<LIMIT>,
        execution: &conduit_body::HostExecutionDelegation,
    ) -> Result<(), ()> {
        if self.native_offer.is_none()
            || !self.resources.admissions().is_empty()
            || !self.resources.assignments().is_empty()
        {
            return Err(());
        }
        let member = self.receipt.as_ref().ok_or(())?;
        let available = crate::config::HEAP_BYTES
            .checked_sub(crate::heap::usage().0)
            .ok_or(())?;
        let (native, _) =
            crate::i2c_installation::prepare(self.resources.host(), available).map_err(|_| ())?;
        let mut slot = Some(native);
        let mut inventory = resident::inventory().map_err(|_| ())?;
        // Native codecs are prepared from the build-checked contract; Source,
        // graph planning and capability-advertisement decoding stay off-device.
        // Flat interface and allocator overhead still need whole-Root peak proof.
        let contract = conduit_i2c_transaction::contract::I2cContract::prepare_bounded(
            crate::config::HEAP_BYTES
                .checked_sub(crate::heap::usage().0)
                .ok_or(())?,
        )
        .map_err(|_| ())?;
        let policy = m::Policy {
            preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
            execution: Some(execution),
            active_worksets: 0,
            work_pool: crate::config::RESIDENT_WORK_POOL,
            maxima: crate::config::MIXED_PREPARATION_MAXIMA,
            maximum_storage_bytes: crate::config::RESIDENT_PREPARATION_STORAGE_BYTES.min(
                crate::config::HEAP_BYTES
                    .checked_sub(crate::heap::usage().0)
                    .ok_or(())?,
            ),
        };
        let context = conduit_body::NativePreparationContext {
            bootstrap: &self.seed,
            member,
            actual: self.resources.host(),
            image_id: crate::config::IMAGE_ID,
            configuration_id: crate::config::CONFIGURATION_ID,
            now_millis: Instant::now().as_millis(),
        };
        let issuer = &mut self.native_issuer;
        let stored = m::prepare_retained_with_issuer(
            &mut inventory,
            &mut slot,
            &contract,
            policy,
            context,
            workspace,
            |approved, native| {
                issuer.issue(context, approved, native, policy.maximum_storage_bytes)
            },
        )
        .map_err(|_| ())?;
        // No live kernel borrows the temporary flat interface. Release it
        // before observation/admission and the execution lifetime.
        drop(contract);
        self.freshness = self.freshness.checked_add(1).ok_or(())?;
        let committed = stored
            .admit_observed(
                &mut self.resources,
                crate::config::RESIDENT_WORK_UNITS,
                self.freshness,
            )
            .map_err(|_| ())?;
        let workset = RefCell::new(crate::mixed_supervision::MixedWorkset::new(
            committed, policy,
        ));
        crate::execution_supervisor::supervise(
            link,
            workspace,
            &self.seed,
            member,
            &mut self.resources,
            &workset,
        )
        .await
    }
}
