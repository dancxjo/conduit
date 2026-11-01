//! Fresh owner State-only seed recovery; no volatile stage or expected reference.
use super::super::super::{debug, Owner};
use super::catalog::seed;
use conduit_core as c;
use conduit_std_host::body_storage_generation::BodyStorageGenerationRequest;
use std::{collections::BTreeMap, path::Path};
pub(crate) struct SourceCatalogRecoveryInput {
    pub(crate) binding: conduit_std_host::state_value::DurableStateBinding,
    pub(crate) value_type: c::StructuredInfoType,
}
impl Owner {
    pub(crate) fn prepare_source_catalog_recovery(
        &mut self,
        root: &Path,
        preparation_limit_bytes: u64,
    ) -> Result<SourceCatalogRecoveryInput, String> {
        if self.host.is_playing()
            || self.session.realization().is_some()
            || self.pending_browser.is_some()
            || self.session.evidence().body.state != conduit_body::BodyState::Lulled
        {
            return Err("catalog recovery requires the original lulled idle owner".into());
        }
        self.ensure_execution_retained(root)?;
        let text =
            include_str!("../../../../../../bodies/environment/profiles/catalog-seed-read.conduit");
        if text.len() as u64 + std::mem::size_of::<SourceCatalogRecoveryInput>() as u64
            > preparation_limit_bytes
        {
            return Err("catalog recovery source preparation volume limit exceeded".into());
        }
        let source = crate::plot_source::parse(text)?;
        let plot = source.expand_entry_for_authoring()?;
        if plot.expanded.gears.len() != 1 {
            return Err("catalog recovery must contain only its canonical keep".into());
        }
        let seed = seed::extract(&plot, self.session.evidence().body_id.as_str())?;
        let input = SourceCatalogRecoveryInput {
            binding: seed.request.binding.clone(),
            value_type: seed.request.value_type.clone(),
        };
        self.host
            .current_mut()
            .select_body_storage_generation(BodyStorageGenerationRequest {
                snapshot: None,
                state: Some(seed.request),
            })?;
        let selected = self
            .host
            .selected_body_storage_generation()
            .ok_or("catalog recovery selected generation is stale")?;
        if selected.state_binding() != Some(&input.binding)
            || selected.snapshot_resource().is_some()
        {
            return Err("catalog recovery selection is not exact State-only custody".into());
        }
        let resident = conduit_body::ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        let boundaries = [(
            conduit_planner::ForeBoundaryKey {
                direction: c::PortDirection::Output,
                front_port_id: "current".into(),
                track: c::ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: input.binding.maximum_value_bytes,
            },
        )]
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        let partition = self.plan_partition_with_limits(
            &plot,
            &resident,
            &[],
            &boundaries,
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: input.binding.maximum_value_bytes,
            },
        )?;
        if partition.plan.fragments.len() != 1 {
            return Err("catalog recovery has foreign fragments".into());
        }
        let fragment = &partition.plan.fragments[0];
        if fragment.states.len() != 1
            || fragment.states[0].state_id != input.binding.state
            || fragment.states[0].value_kind != input.binding.value_kind
            || fragment.states[0].maximum_value_bytes != input.binding.maximum_value_bytes
            || fragment.states[0].lifetime != c::StateLifetime::Body
            || fragment
                .placements
                .iter()
                .any(|gear| !gear.authority.is_empty())
        {
            return Err("catalog recovery Plan differs from exact grant-free State binding".into());
        }
        let profile = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE
            .with_state_storage(1, input.binding.maximum_value_bytes)
            .map_err(debug)?;
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment_for_profile_from_plan(
            &partition.plan,
            &fragment.fragment_id,
            profile,
        )
        .map_err(debug)?;
        if lowered.states.len() != 1 || lowered.states[0].contract.state_id != input.binding.state {
            return Err("catalog recovery lowered State differs".into());
        }
        if self.resident.as_ref() != Some(&resident) {
            let revision = self.session.evidence().body.workload_revision;
            let checked = self
                .resident
                .as_ref()
                .ok_or("catalog recovery has no current resident")?
                .checked_plot_id
                .as_str()
                .to_owned();
            self.replace_plot(root, text, revision, &checked)?;
        }
        let ad = self.host.advertisement();
        self.session
            .propose(vec![partition], &ad.host_id, &ad.boot_id)
            .map_err(debug)?;
        self.persist(root)?;
        Ok(input)
    }
}
#[cfg(test)]
#[path = "recovery/tests.rs"]
mod tests;
