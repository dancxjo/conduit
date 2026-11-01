//! Ordinary same-Body catalog proposal, gated by actual admitted chunk readback.
//! Preparation is neither catalog commitment nor acknowledgement of the prefix.
use super::super::super::{debug, Owner};
use conduit_core as c;
use conduit_std_host::body_storage_generation::{
    BodyStorageGenerationRequest, BodyStorageSnapshotRequest, BodyStorageStateRequest,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path};

pub(crate) struct SourceCatalogInput {
    pub(crate) input: conduit_std_host::ExternalForeInput,
    pub(crate) reference: c::BoundedResourceRef,
    pub(crate) authority: c::AuthorityGrant,
    pub(crate) expected_current: Vec<u8>,
    pub(crate) binding: conduit_std_host::state_value::DurableStateBinding,
}
impl Owner {
    pub(crate) fn prepare_source_recording_catalog(
        &mut self,
        root: &Path,
        snapshot_authority: &c::AuthorityGrant,
        preparation_limit_bytes: u64,
    ) -> Result<SourceCatalogInput, String> {
        if self.host.is_playing()
            || self.session.realization().is_some()
            || self.pending_browser.is_some()
            || self.session.evidence().body.state != conduit_body::BodyState::Lulled
        {
            return Err("Source catalog requires the original lulled idle owner".into());
        }
        self.ensure_execution_retained(root)?;
        let stage = self
            .source_recording_stage
            .as_ref()
            .ok_or("Source recording stage is absent")?;
        if stage.body_id != self.session.evidence().body_id {
            return Err("Source catalog belongs to another Body".into());
        }
        let verified = stage.verified_catalog_references()?;
        let storage = self
            .host
            .selected_body_storage()
            .ok_or("Source catalog has no selected directory")?;
        let catalog = conduit_environment_runtime::recording::PreparedRecordingCatalog::prepare(
            &verified,
            storage.bounds().maximum_snapshot_records as usize,
            storage.bounds().total_disk_bytes,
        )
        .map_err(|e| format!("Source catalog: {e:?}"))?;
        let content = catalog.content();
        let digest: [u8; 32] = Sha256::digest(content.as_bytes()).into();
        let reference = c::BoundedResourceRef {
            identity: c::ResourceSemanticIdentity::from_digest(digest),
            content_profile: "text/json-utf8@1".into(),
            access_class: "resource/snapshot@1".into(),
            extent: c::ResourceExtent {
                bytes: content.len() as u64,
                items: Some(1),
            },
            lifetime: c::ResourceLifetime {
                version: c::ResourceVersionIdentity::from_digest(digest),
                expires_at: None,
            },
        };
        reference
            .validate()
            .map_err(|e| format!("catalog reference: {e:?}"))?;
        let encoded = reference
            .encode()
            .map_err(|e| format!("catalog reference: {e:?}"))?;
        let hex: String = encoded.iter().map(|byte| format!("{byte:02x}")).collect();
        let text = include_str!(
            "../../../../../../bodies/environment/profiles/catalog-checkpoint.conduit"
        )
        .replace("@REFERENCE@", &hex);
        if content.len() > 4096
            || content.len() as u64 + text.len() as u64 + encoded.len() as u64
                > preparation_limit_bytes
        {
            return Err("Source catalog input/source preparation volume limit exceeded".into());
        }
        let source = crate::plot_source::parse(&text)?;
        let plot = source.expand_entry_for_authoring()?;
        let seed = seed::extract(&plot, self.session.evidence().body_id.as_str())?;
        let mut programs = Vec::new();
        for gear in &plot.expanded.gears {
            if gear.kind_id.as_str() == c::STATE_VALUE_KIND
                || gear.kind_id.as_str() == conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND
            {
                continue;
            }
            let encoded = gear
                .configuration
                .iter()
                .find_map(|entry| match (&*entry.key, &entry.value) {
                    ("program", c::ConfigurationValue::Text(value)) => Some(value),
                    _ => None,
                })
                .ok_or("catalog has an unexpected non-State/non-publication Gear")?;
            let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
                .map_err(|e| format!("catalog expression: {e:?}"))?;
            let temporal = gear
                .inputs
                .first()
                .ok_or("catalog expression input absent")?
                .temporal;
            programs.push((program, temporal));
        }
        if programs.len() != 1 {
            return Err(
                "catalog requires its one exact authored present-reference expression".into(),
            );
        }
        let leaf = c::StructuredInfoValue::leaf(
            c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID))
                .map_err(debug)?,
            encoded,
        )
        .map_err(debug)?;
        let expected_current =
            c::StructuredInfoValue::variant(seed.request.value_type.clone(), "some", leaf)
                .map_err(debug)?
                .canonical_bytes()
                .map_err(debug)?;
        let binding = seed.request.binding.clone();
        let ad = self.host.advertisement();
        if snapshot_authority.host_id != ad.host_id || snapshot_authority.boot_id != ad.boot_id {
            return Err("catalog authority belongs to another Host/Boot".into());
        }
        let input = SourceCatalogInput {
            input: conduit_std_host::ExternalForeInput {
                front_port_id: "catalog".into(),
                track: c::ConnectionTrack::Payload,
                bytes: content.as_bytes().to_vec(),
            },
            reference,
            authority: snapshot_authority.clone(),
            expected_current,
            binding,
        };
        // Expression refresh precedes storage selection so the selected storage
        // descriptor is current at the final ordinary proposal generation.
        self.host
            .current_mut()
            .select_expression_programs(&programs, preparation_limit_bytes)?;
        self.host
            .current_mut()
            .select_body_storage_generation(BodyStorageGenerationRequest {
                snapshot: Some(BodyStorageSnapshotRequest {
                    reference: input.reference.clone(),
                    access: c::ResourceAccessMode::WriteCandidatePublish,
                }),
                state: Some(seed.request),
            })?;
        let host = self.host.current();
        let generation = host
            .selected_body_storage_generation()
            .ok_or("catalog selected generation is stale")?;
        if generation.state_binding() != Some(&input.binding)
            || generation.snapshot_reference() != Some(&input.reference)
        {
            return Err("catalog selected State/reference differs".into());
        }
        let resource = generation
            .snapshot_resource()
            .ok_or("catalog snapshot resource absent")?;
        let expected = conduit_std_host::linux_snapshot::offer_for_resource(resource)?;
        let [requirement] = expected.authority_requirements.as_slice() else {
            return Err("catalog snapshot authority profile differs".into());
        };
        let ad = host.advertisement();
        if !ad.capabilities.iter().any(|offer| offer == &expected)
            || input.authority.grant_id.as_str().is_empty()
            || input.authority.capability_id != expected.capability_id
            || input.authority.host_id != ad.host_id
            || input.authority.boot_id != ad.boot_id
            || input.authority.contract_id != requirement.contract_id
            || input.authority.host_call_contract_id != requirement.host_call_contract_id
            || input.authority.subject_kind != requirement.subject_kind
        {
            return Err(
                "catalog requires independent exact selected snapshot publication authority".into(),
            );
        }
        let resident = conduit_body::ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        let boundaries = [
            (c::PortDirection::Input, "catalog", 4096),
            (
                c::PortDirection::Output,
                "current",
                input.binding.maximum_value_bytes,
            ),
        ]
        .into_iter()
        .map(|(direction, name, bytes)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: c::ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
        let partition = self.plan_partition_with_limits(
            &plot,
            &resident,
            std::slice::from_ref(&input.authority),
            &boundaries,
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: input.binding.maximum_value_bytes,
            },
        )?;
        let states: Vec<_> = partition
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.states)
            .collect();
        if states.len() != 1
            || states[0].state_id != input.binding.state
            || states[0].value_kind != input.binding.value_kind
            || states[0].maximum_value_bytes != input.binding.maximum_value_bytes
            || states[0].lifetime != c::StateLifetime::Body
        {
            return Err("catalog sealed keep differs from selected stable binding".into());
        }
        let profile = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE
            .with_state_storage(1, input.binding.maximum_value_bytes)
            .map_err(debug)?;
        let fragment = partition
            .plan
            .fragments
            .first()
            .ok_or("catalog has no local fragment")?;
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment_for_profile_from_plan(
            &partition.plan,
            &fragment.fragment_id,
            profile,
        )
        .map_err(debug)?;
        if lowered.states.len() != 1
            || lowered.states[0].contract.state_id != input.binding.state
            || lowered.states[0].contract.value_kind != input.binding.value_kind
            || lowered.states[0].contract.maximum_value_bytes != input.binding.maximum_value_bytes
        {
            return Err("catalog lowered State differs from selected stable binding".into());
        }
        if self.resident.as_ref() != Some(&resident) {
            let revision = self.session.evidence().body.workload_revision;
            let checked = self
                .resident
                .as_ref()
                .ok_or("catalog has no current resident")?
                .checked_plot_id
                .as_str()
                .to_owned();
            self.replace_plot(root, &text, revision, &checked)?;
        }
        let ad = self.host.advertisement();
        self.session
            .propose(vec![partition], &ad.host_id, &ad.boot_id)
            .map_err(debug)?;
        self.persist(root)?;
        Ok(input)
    }
}
#[path = "catalog/seed.rs"]
pub(super) mod seed;
#[cfg(test)]
#[path = "catalog/tests.rs"]
mod tests;
