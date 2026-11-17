//! Bind only the actual installed source gates after durable owner acknowledgment.
use super::BodyKernel;
impl BodyKernel<'_> {
    pub(crate) fn requires_startup(&self) -> bool {
        !self.startup_gates.is_empty()
    }
    pub(crate) fn bind_startup(
        &self,
        startup: Option<&crate::body_execution::CommittedBodyStartup>,
    ) -> Result<(), String> {
        if self.startup_gates.is_empty() {
            return Ok(());
        }
        let startup =
            startup.ok_or("body/wake requires the exact durable Started acknowledgment")?;
        let eligible = startup.eligible(conduit_body::StartupScope::Wake);
        for gate in &self.startup_gates {
            gate.bind(eligible)?;
        }
        Ok(())
    }
}

impl BodyKernel<'_> {
    pub(crate) fn prepare_startup_outputs(
        &mut self,
        parts: &[conduit_body::BodyPlotPlan],
        selected: Option<&crate::hosted_audio::HostedPlaybackSelection>,
    ) -> Result<(), String> {
        use super::InstalledBack;
        use crate::body_execution::StartupChimeCallReceipt;
        let count = parts
            .iter()
            .flat_map(|part| &part.plan.fragments)
            .flat_map(|fragment| &fragment.placements)
            .filter(|gear| {
                gear.implementation_id.as_str() == conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION
            })
            .count();
        if !self.startup_outputs.is_empty() || !self.startup_receipts.is_empty() {
            return Err("startup providers were already prepared".into());
        }
        self.startup_outputs
            .try_reserve_exact(count)
            .map_err(|_| "startup provider slots unavailable")?;
        self.startup_receipts
            .try_reserve_exact(count)
            .map_err(|_| "startup receipt slots unavailable")?;
        for part in parts {
            for fragment in &part.plan.fragments {
                let map = self
                    .partitions
                    .iter()
                    .find(|map| {
                        map.fragment_id == fragment.fragment_id && map.plan_id == fragment.plan_id
                    })
                    .ok_or("startup original partition map absent")?;
                for gear in &fragment.placements {
                    if gear.implementation_id.as_str()
                        != conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION
                    {
                        continue;
                    }
                    let node = map
                        .node_for_placement(&gear.placement_id)
                        .ok_or("startup selected node absent")?;
                    let Some(InstalledBack::StartupChime(back)) =
                        self.scheduler.drivers().get(usize::from(node.0))
                    else {
                        return Err("startup selected node has different installed Back".into());
                    };
                    let output = crate::hosted_audio::startup_chime::PreparedChimeOutput::prepare(
                        gear,
                        selected,
                        back.pcm(),
                    )?;
                    self.startup_outputs.push((node, output));
                    self.startup_receipts.push(StartupChimeCallReceipt {
                        node,
                        request: None,
                        disposition: None,
                        failure: None,
                        kernel_accepted: false,
                    });
                }
            }
        }
        Ok(())
    }
}

/// The one installed finite Bool cue operation, never a generic Host Call bypass.
pub(super) fn supports(operation: &super::LoweredHostCall) -> bool {
    operation.contract_id.as_str() == conduit_std_offers::STARTUP_CHIME_OPERATION
        && operation
            .target_kind
            .as_ref()
            .is_some_and(|kind| kind.as_str() == conduit_core::BOOL_INFO_ID)
        && operation.maximum_in_flight == 1
        && operation.binding.maximum_input_bytes == conduit_core::BOOL_ENCODED_LEN as u32
        && operation.binding.maximum_output_bytes == 0
}
