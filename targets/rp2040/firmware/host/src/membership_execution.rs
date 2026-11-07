//! Persistent Root custody, independent of one carrier or its acknowledgments.
use super::*;
use crate::resident_preparation as resident;
use core::cell::RefCell;
use embassy_time::Instant;
#[path = "resident_supervision.rs"]
mod resident_supervision;

impl MembershipState {
    pub(super) async fn run_retained(
        &mut self,
        link: &mut Link,
        workspace: &conduit_body::HostCommandInput<LIMIT>,
        policy: &conduit_body::HostExecutionDelegation,
    ) -> Result<(), ()> {
        let member = self.receipt.as_ref().ok_or(())?;
        if !self.resources.admissions().is_empty() || !self.resources.assignments().is_empty() {
            return Err(());
        }
        let context = conduit_body::NativePreparationContext {
            bootstrap: &self.seed,
            member,
            actual: self.resources.host(),
            image_id: crate::config::IMAGE_ID,
            configuration_id: crate::config::CONFIGURATION_ID,
            now_millis: Instant::now().as_millis(),
        };
        let profile = crate::execution_selection::select::<
            LIMIT,
            { crate::resident::generated::ENTRY_COUNT },
        >(
            workspace,
            context,
            crate::config::PREPARATION_DELEGATION.as_ref(),
            Some(policy),
        )
        .map_err(|_| ())?;
        if profile == crate::execution_selection::Profile::MixedI2c {
            return self.run_mixed(link, workspace, policy).await;
        }
        let mut inventory = resident::inventory().map_err(|_| ())?;
        let stored = resident::prepare_retained_execution_storage(
            &mut inventory,
            crate::config::PREPARATION_DELEGATION.as_ref(),
            resident::ExecutionPolicy {
                installed: Some(policy),
                active_worksets: 0,
            },
            conduit_body::NativePreparationContext {
                bootstrap: &self.seed,
                member,
                actual: self.resources.host(),
                image_id: crate::config::IMAGE_ID,
                configuration_id: crate::config::CONFIGURATION_ID,
                now_millis: Instant::now().as_millis(),
            },
            workspace,
            crate::config::RESIDENT_PREPARATION_MAXIMA,
            crate::config::RESIDENT_PREPARATION_STORAGE_BYTES,
        )
        .map_err(|_| ())?;
        self.freshness = self.freshness.checked_add(1).ok_or(())?;
        let committed = stored
            .admit_observed(
                &mut self.resources,
                crate::config::RESIDENT_WORK_POOL,
                crate::config::RESIDENT_WORK_UNITS,
                self.freshness,
                crate::config::RESIDENT_PREPARATION_STORAGE_BYTES,
            )
            .map_err(|_| ())?;
        let (fragments, original_stamp) = committed.into_parts();
        let workset = RefCell::new(resident_supervision::ResidentWorkset::new(
            fragments,
            original_stamp,
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
