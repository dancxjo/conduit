//! Exact input, terminal and durable residence admission before Body Play.
use super::super as installed;
use super::*;

impl<'a> BodyKernel<'a> {
    /// Prepare input origin descriptors without granting issued-Play authority.
    pub(crate) fn prepare_input_origin(
        &mut self,
        advertisement: &conduit_core::HostAdvertisement,
        plan: &conduit_body::BodyPlan,
        prospective: &conduit_body::BodyPlayIdentity,
    ) -> Result<(), String> {
        if !installed::keyboard_input_binding::has_input_source(&self.operations) {
            // Ordinary non-input admission retains its own refusals and ordering.
            return Ok(());
        }
        if self.input_origin.is_some() {
            return Err("Body input origin was prepared twice".into());
        }
        self.input_origin = Some(
            installed::keyboard_input_binding::KeyboardRequestAdmission::prepare_body(
                advertisement,
                plan,
                prospective,
                &self.partitions,
                &self.operations,
            )?,
        );
        if self.supported_parsec_input_scan {
            self.input_origin
                .as_mut()
                .ok_or("Parsec input origin is absent")?
                .enable_completed_event_custody()?;
            self.parsec_map = Some(parsec_acknowledged_map::ParsecAcknowledgedMap::prepare(
                plan,
                advertisement,
                prospective,
                self.partitions
                    .first()
                    .ok_or("Parsec map partition is absent")?,
            )?);
        }
        Ok(())
    }

    pub(crate) fn confirm_input_play(
        &mut self,
        advertisement: &conduit_core::HostAdvertisement,
        issued: &conduit_body::BodyPlayIdentity,
    ) -> Result<(), String> {
        match self.input_origin.as_mut() {
            Some(origin) => origin.confirm_body_play(advertisement, issued),
            None if !installed::keyboard_input_binding::has_input_source(&self.operations) => {
                Ok(())
            }
            None => Err("Body input origin was not prepared".into()),
        }?;
        if let Some(map) = self.parsec_map.as_mut() {
            map.confirm(advertisement, issued)?;
        }
        Ok(())
    }

    pub(crate) fn attach_parsec_terminal(
        &mut self,
        plan: &conduit_body::BodyPlan,
        host: &conduit_core::HostAdvertisement,
        columns: usize,
        rows: usize,
    ) -> Result<(), String> {
        if !self.supported_parsec_input_scan
            || self.parsec_map.is_none()
            || self.parsec_terminal.is_some()
        {
            return Err("terminal attachment requires the exact prepared Parsec input Body".into());
        }
        self.parsec_terminal = Some(parsec_terminal_owner::ParsecTerminalOwner::prepare(
            plan, host, columns, rows,
        )?);
        Ok(())
    }

    /// Bind at most one selected reader and publisher to their exact lowered
    /// placements before Play. No ambient storage fallback is available.
    pub(crate) fn attach_todo_checkpoint(
        &mut self,
        partitions: &[BodyPlotPlan],
        root: &std::path::Path,
        checkpoint: crate::todo_durable_resource::CheckpointIdentity,
    ) -> Result<(), String> {
        let mut selected = 0;
        for (partition, body_plot) in partitions.iter().enumerate() {
            let lowered = self
                .partitions
                .get(partition)
                .ok_or("Todo checkpoint partition missing")?;
            for placement in body_plot
                .plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
            {
                match placement.implementation_id.as_str() {
                    conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION => {
                        if self.todo_checkpoint.is_some() {
                            return Err("duplicate Todo checkpoint publisher".into());
                        }
                        self.todo_checkpoint = Some(
                            crate::todo_checkpoint_call::TodoCheckpointHost::prepare(
                                root,
                                placement,
                                lowered,
                                checkpoint.clone(),
                            )
                            .map_err(|error| {
                                format!("prepare selected Todo checkpoint: {error:?}")
                            })?,
                        );
                        selected += 1;
                    }
                    conduit_std_offers::TODO_CHECKPOINT_READ_IMPLEMENTATION => {
                        if self.todo_checkpoint_read.is_some() {
                            return Err("duplicate Todo checkpoint reader".into());
                        }
                        self.todo_checkpoint_read = Some(
                            crate::todo_checkpoint_read_call::TodoCheckpointReadHost::prepare(
                                root,
                                placement,
                                lowered,
                                checkpoint.clone(),
                            )
                            .map_err(|error| {
                                format!("prepare selected Todo checkpoint read: {error:?}")
                            })?,
                        );
                        selected += 1;
                    }
                    _ => {}
                }
            }
        }
        if selected == 0 {
            return Err("Body has no selected Todo checkpoint placement".into());
        }
        Ok(())
    }

    /// Exact finite pure scans and the separately validated live Parsec input
    /// profile have installed Body routes. This admits execution shape only;
    /// Face action grants and current input-map authority remain separate.
    pub(crate) fn require_supported_execution(&self) -> Result<(), String> {
        if self.operations.iter().any(|op| {
            op.contract_id.as_str()
                == conduit_body_make::deployment_runtime_catalog::CRECHE_HOST_CALL
        }) && self.creche.is_none()
        {
            return Err("planned Creche has no exact admitted Host encounter owner".into());
        }
        if self.operations.iter().any(|operation| {
            operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_PUBLISH_CALL
        }) && self.todo_checkpoint.is_none()
        {
            return Err("planned Todo checkpoint has no selected durable Host residence".into());
        }
        if self.operations.iter().any(|operation| {
            operation.contract_id.as_str() == conduit_std_offers::TODO_CHECKPOINT_READ_CALL
        }) && self.todo_checkpoint_read.is_none()
        {
            return Err(
                "planned Todo checkpoint read has no selected durable Host residence".into(),
            );
        }
        if self
            .activations
            .iter()
            .any(|bound| !bound.entries.is_empty())
            && !self.supported_pure_activation
            && !self.supported_parsec_input_scan
        {
            return Err("Body activation coordinator is not installed".into());
        }
        Ok(())
    }
}
