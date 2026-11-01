//! Admitted reads selected only from private recovered State/catalog custody.
use super::super::super::{debug, Owner};
use conduit_core as c;
use conduit_std_host::body_storage_generation::{
    BodyStorageGenerationRequest, BodyStorageSnapshotRequest,
};
use std::{collections::BTreeMap, path::Path};
pub(crate) struct RecoveredSourceReadInput {
    pub(crate) input: conduit_std_host::ExternalForeInput,
    pub(crate) reference: c::BoundedResourceRef,
    pub(crate) authority: c::AuthorityGrant,
    pub(crate) chunk_index: Option<usize>,
}
impl Owner {
    /// Prepare an ordinary same-Body read. Private recovered custody selects the
    /// reference; the caller independently supplies current selected read authority.
    /// Later refusal may leave a prepared generation/resident; no rollback,
    /// successful read, ACK or producer retirement is implied by preparation.
    pub(crate) fn prepare_recovered_source_read(
        &mut self,
        root: &Path,
        read_grant: &c::AuthorityGrant,
        chunk_index: Option<usize>,
        preparation_limit_bytes: u64,
    ) -> Result<RecoveredSourceReadInput, String> {
        if self.host.is_playing()
            || self.session.realization().is_some()
            || self.pending_browser.is_some()
            || self.session.evidence().body.state != conduit_body::BodyState::Lulled
        {
            return Err("recovered Source read requires the original lulled idle owner".into());
        }
        self.ensure_execution_retained(root)?;
        let recovered = self
            .recovered_source_catalog
            .as_ref()
            .ok_or("recovered catalog State reference is absent")?;
        let ad = self.host.advertisement();
        let play = recovered.play();
        if recovered.binding().body.as_str() != self.session.evidence().body_id.as_str()
            || play.body_id != self.session.evidence().body_id
            || recovered.host_id() != &ad.host_id
            || recovered.boot_id() != &ad.boot_id
        {
            return Err("recovered catalog belongs to another Body or Host Boot".into());
        }
        if read_grant.host_id != ad.host_id || read_grant.boot_id != ad.boot_id {
            return Err("recovered Source read grant belongs to another Host/Boot".into());
        }
        let reference = if let Some(index) = chunk_index {
            let contents = self
                .recovered_source_catalog_contents
                .as_ref()
                .ok_or("admitted recovered catalog content is absent")?;
            if contents.catalog_reference() != recovered.reference() {
                return Err("recovered catalog contents differ from the State reference".into());
            }
            contents
                .references()
                .get(index)
                .ok_or("recovered Source chunk index exceeds the verified catalog")?
                .clone()
        } else {
            recovered.reference().clone()
        };
        if reference.extent.bytes == 0
            || reference.extent.bytes > 4096
            || reference.extent.items != Some(1)
            || reference.access_class.as_str() != "resource/snapshot@1"
            || reference.content_profile.as_str() != "text/json-utf8@1"
            || reference.lifetime.expires_at.is_some()
        {
            return Err("recovered Source reference exceeds its finite snapshot profile".into());
        }
        // Check finite reference/source volume before serialization/template allocation.
        let template = include_str!(
            "../../../../../../bodies/environment/profiles/recording-readback.conduit"
        );
        let encoded_len = reference.encoded_len().map_err(debug)?;
        if encoded_len > 512
            || (encoded_len as u64) * 3
                + template.len() as u64
                + std::mem::size_of::<RecoveredSourceReadInput>() as u64
                > preparation_limit_bytes
        {
            return Err("recovered Source read preparation volume limit exceeded".into());
        }
        let encoded = reference.encode().map_err(debug)?;
        let hex: String = encoded.iter().map(|b| format!("{b:02x}")).collect();
        let text = template.replace("@REFERENCE@", &hex);
        let plot = crate::plot_source::parse(&text)?.expand_entry_for_authoring()?;
        let resident = conduit_body::ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        self.host
            .current_mut()
            .select_body_storage_generation(BodyStorageGenerationRequest {
                snapshot: Some(BodyStorageSnapshotRequest {
                    reference: reference.clone(),
                    access: c::ResourceAccessMode::ReadPublished,
                }),
                state: None,
            })?;
        let host = self.host.current();
        let selected = host
            .selected_body_storage_generation()
            .ok_or("recovered Source read generation is stale")?;
        let resource = selected
            .snapshot_resource()
            .ok_or("recovered Source selected snapshot is absent")?;
        if selected.snapshot_reference() != Some(&reference)
            || resource.content.as_ref().is_none_or(|content| {
                content.contract.access != c::ResourceAccessMode::ReadPublished
            })
        {
            return Err("recovered Source selected reference or access differs".into());
        }
        let expected = conduit_std_host::linux_snapshot::offer_for_resource(resource)?;
        let [required] = expected.authority_requirements.as_slice() else {
            return Err("recovered Source read authority profile differs".into());
        };
        let ad = host.advertisement();
        if !ad.capabilities.iter().any(|offer| offer == &expected)
            || read_grant.grant_id.as_str().is_empty()
            || read_grant.capability_id != expected.capability_id
            || read_grant.host_id != ad.host_id
            || read_grant.boot_id != ad.boot_id
            || read_grant.contract_id != required.contract_id
            || read_grant.host_call_contract_id != required.host_call_contract_id
            || read_grant.subject_kind != required.subject_kind
        {
            return Err(
                "recovered Source read requires independent exact selected-generation authority"
                    .into(),
            );
        }
        let boundaries = [
            (c::PortDirection::Input, "reference", 512),
            (c::PortDirection::Output, "content", 4096),
        ]
        .into_iter()
        .map(|(direction, name, byte_capacity)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: c::ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
        let partition = self.plan_partition_with_limits(
            &plot,
            &resident,
            std::slice::from_ref(read_grant),
            &boundaries,
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4096,
            },
        )?;
        if self.resident.as_ref() != Some(&resident) {
            let revision = self.session.evidence().body.workload_revision;
            let checked = self
                .resident
                .as_ref()
                .ok_or("recovered Source read has no resident")?
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
        Ok(RecoveredSourceReadInput {
            input: conduit_std_host::ExternalForeInput {
                front_port_id: "reference".into(),
                track: c::ConnectionTrack::Payload,
                bytes: encoded,
            },
            reference,
            authority: read_grant.clone(),
            chunk_index,
        })
    }
}
#[cfg(test)]
#[path = "recovered_read/tests.rs"]
mod tests;
