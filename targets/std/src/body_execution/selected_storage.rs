//! Use the retained storage generation and its original descriptor owner.
//! Callers supply independent authority, never another root or ResourceOffer.
use super::{
    BodyBorrowedForeOutputAdapter, BodyDurableStateSelection, BodyRunReport, BodyRunRequest,
    BodySnapshotSelection,
};
use crate::{ExternalForeInput, StdHost, TimerAdapter};
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_core::AuthorityGrant;
use std::{io::Write, sync::Arc};

impl StdHost {
    /// Execute a selected snapshot, State, or combined checkpoint. The original Body kernel still reserves the full Plan and
    /// checks its issued Play before any filesystem effect. A successful Fore
    /// delivery is not a durable recording acknowledgment.
    #[allow(clippy::too_many_arguments)]
    pub fn run_body_plan_with_selected_storage_borrowed_to_with_start<
        W: Write,
        T: TimerAdapter,
        F,
    >(
        &mut self,
        request: BodyRunRequest<'_>,
        inputs: &[ExternalForeInput],
        output_fore: &mut dyn BodyBorrowedForeOutputAdapter,
        state_authorities: &[AuthorityGrant],
        snapshot_authority: Option<&AuthorityGrant>,
        output: &mut W,
        timer: &mut T,
        started: F,
    ) -> Result<BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &Wake) -> Result<(), String>,
    {
        let generation = self
            .selected_body_storage_generation()
            .ok_or("Body storage has no current selected generation")?;
        let mut states = request
            .plan
            .plots
            .iter()
            .flat_map(|partition| partition.plan.fragments.iter())
            .flat_map(|fragment| fragment.states.iter());
        let state_contract = states.next();
        if states.next().is_some() {
            return Err("one selected storage generation admits one State".into());
        }
        let state = if let Some(contract) = state_contract {
            let binding = generation
                .state_binding()
                .ok_or("selected Body storage has no exact State binding")?;
            if binding.body != request.plan.body_id.as_str()
                || binding.state != contract.state_id
                || binding.value_kind != contract.value_kind
                || binding.maximum_value_bytes != contract.maximum_value_bytes
            {
                return Err("Body State differs from exact selected storage binding".into());
            }
            Some(
                generation
                    .state_resource()
                    .ok_or("selected Body storage generation has no State residence")?
                    .clone(),
            )
        } else {
            if !state_authorities.is_empty() {
                return Err("State authority supplied for a Plan without State".into());
            }
            None
        };
        let snapshot = generation.snapshot_resource().cloned();
        if snapshot_authority.is_some() && snapshot.is_none() {
            return Err("snapshot authority has no current selected storage resource".into());
        }
        let storage = Arc::clone(
            self.selected_body_storage
                .as_ref()
                .ok_or("selected Body storage descriptor owner is absent")?,
        );
        let body_id = &request.plan.body_id;
        let plan_id = &request.plan.plan_id;
        let snapshot = snapshot
            .as_ref()
            .zip(snapshot_authority)
            .map(|(resource, authority)| BodySnapshotSelection {
                directory: storage.snapshot_directory(),
                resource,
                authority,
                body_id,
                plan_id,
            });
        // The cloned Arc retains these exact descriptors through synchronous
        // execution and cleanup, including every error/unwind path. No ambient
        // canonical path is reopened and no second reservation is introduced.
        if let Some(state) = state.as_ref() {
            self.run_body_plan_with_state_storage_borrowed_to_with_start(
                request,
                inputs,
                output_fore,
                super::BodyStateStorageSelection {
                    selected_root: Some(storage.root_descriptor()),
                    selection: BodyDurableStateSelection {
                        root: storage.state_root(),
                        resource: state,
                        authorities: state_authorities,
                        body_id,
                        plan_id,
                    },
                },
                snapshot,
                output,
                timer,
                started,
            )
        } else {
            let snapshot = snapshot.ok_or("Body storage execution selected no operation")?;
            self.run_body_plan_with_snapshot_borrowed_to_with_start(
                request,
                inputs,
                output_fore,
                snapshot,
                output,
                timer,
                started,
            )
        }
    }
}
