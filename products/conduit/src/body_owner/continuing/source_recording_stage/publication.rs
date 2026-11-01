//! Ordinary owner publication preparation on the original selected Host.
//! This proposes the actual resident workset; it publishes no bytes or ACK.
use super::super::super::{debug, Owner};
use super::*;
use conduit_core as c;
use conduit_std_host::body_storage_generation::{
    BodyStorageGenerationRequest, BodyStorageSnapshotRequest,
};
use std::{collections::BTreeMap, path::Path};

pub(crate) struct SourcePublicationInput {
    pub(crate) input: conduit_std_host::ExternalForeInput,
    pub(crate) reference: c::BoundedResourceRef,
    pub(crate) authority: c::AuthorityGrant,
    pub(crate) expected_output: Vec<u8>,
    pub(crate) readback: bool,
}
impl Owner {
    /// Select exact storage and install the ordinary same-Body publication
    /// proposal. A later worker must perform it with the actual start handshake.
    /// Failures after selection/replacement may leave that prepared generation
    /// or resident in place; no success/rollback/commit or retirement is implied.
    pub(crate) fn prepare_source_recording_publication(
        &mut self,
        root: &Path,
        publish_grant: &c::AuthorityGrant,
        preparation_limit_bytes: u64,
    ) -> Result<SourcePublicationInput, String> {
        self.prepare_source_recording_operation(root, publish_grant, preparation_limit_bytes, false)
    }
    /// Read the exact retained chunk with an independent read grant. This also
    /// permits recovery of a publication whose reply was lost; it proves no
    /// catalog commitment and does not retire staged records.
    pub(crate) fn prepare_source_recording_readback(
        &mut self,
        root: &Path,
        read_grant: &c::AuthorityGrant,
        preparation_limit_bytes: u64,
    ) -> Result<SourcePublicationInput, String> {
        self.prepare_source_recording_operation(root, read_grant, preparation_limit_bytes, true)
    }
    fn prepare_source_recording_operation(
        &mut self,
        root: &Path,
        publish_grant: &c::AuthorityGrant,
        preparation_limit_bytes: u64,
        readback: bool,
    ) -> Result<SourcePublicationInput, String> {
        if self.host.is_playing()
            || self.session.realization().is_some()
            || self.pending_browser.is_some()
            || self.session.evidence().body.state != conduit_body::BodyState::Lulled
        {
            return Err("Source publication requires the original lulled idle owner".into());
        }
        self.ensure_execution_retained(root)?;
        let stage = self
            .source_recording_stage
            .as_ref()
            .ok_or("Source recording stage is absent")?;
        if stage.body_id != self.session.evidence().body_id {
            return Err("Source recording belongs to another Body".into());
        }
        let prepared = stage
            .prepared
            .as_ref()
            .ok_or("Source checkpoint input is not prepared")?;
        let content = prepared
            .snapshot
            .content()
            .map_err(|e| format!("Source publication content: {e:?}"))?;
        let expected_published = prepared
            .reference
            .encode()
            .map_err(|e| format!("Source publication reference: {e:?}"))?;
        if content.is_empty() || content.len() > 4096 || expected_published.len() > 512 {
            return Err("Source publication input exceeds its finite snapshot profile".into());
        }
        let hex: String = expected_published
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let template = if readback {
            include_str!("../../../../../../bodies/environment/profiles/recording-readback.conduit")
        } else {
            include_str!(
                "../../../../../../bodies/environment/profiles/recording-publication.conduit"
            )
        };
        let text = template.replace("@REFERENCE@", &hex);
        // Named input/source volume, not an allocator or whole-Root admission.
        let volume = content.len() as u64
            + expected_published.len() as u64
            + text.len() as u64
            + std::mem::size_of::<SourcePublicationInput>() as u64;
        if volume > preparation_limit_bytes {
            return Err("Source publication input preparation limit exceeded".into());
        }
        let source = crate::plot_source::parse(&text)?;
        let plot = source.expand_entry_for_authoring()?;
        let resident = conduit_body::ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        );
        let input = SourcePublicationInput {
            input: conduit_std_host::ExternalForeInput {
                front_port_id: if readback { "reference" } else { "chunk" }.into(),
                track: c::ConnectionTrack::Payload,
                bytes: if readback {
                    expected_published.clone()
                } else {
                    content.to_vec()
                },
            },
            reference: prepared.reference.clone(),
            authority: publish_grant.clone(),
            expected_output: if readback {
                content.to_vec()
            } else {
                expected_published
            },
            readback,
        };
        let ad = self.host.advertisement();
        if publish_grant.host_id != ad.host_id || publish_grant.boot_id != ad.boot_id {
            return Err("Source publication grant belongs to another Host/Boot".into());
        }
        let access = if readback {
            c::ResourceAccessMode::ReadPublished
        } else {
            c::ResourceAccessMode::WriteCandidatePublish
        };
        self.host
            .current_mut()
            .select_body_storage_generation(BodyStorageGenerationRequest {
                snapshot: Some(BodyStorageSnapshotRequest {
                    reference: input.reference.clone(),
                    access,
                }),
                state: None,
            })?;
        let host = self.host.current();
        let selected = host
            .selected_body_storage_generation()
            .ok_or("Source publication selected generation is stale")?;
        let resource = selected
            .snapshot_resource()
            .ok_or("Source publication selected snapshot is absent")?;
        if selected.snapshot_reference() != Some(&input.reference)
            || resource
                .content
                .as_ref()
                .is_none_or(|content| content.contract.access != access)
        {
            return Err("Source publication selected reference or access differs".into());
        }
        let expected = conduit_std_host::linux_snapshot::offer_for_resource(resource)?;
        let [requirement] = expected.authority_requirements.as_slice() else {
            return Err("Source publication authority profile differs".into());
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
                "Source publication requires independent exact selected-generation authority"
                    .into(),
            );
        }
        let mut boundaries = BTreeMap::new();
        for (direction, name, bytes) in [
            (
                c::PortDirection::Input,
                if readback { "reference" } else { "chunk" },
                if readback { 512 } else { 4096 },
            ),
            (
                c::PortDirection::Output,
                if readback { "content" } else { "published" },
                if readback { 4096 } else { 512 },
            ),
        ] {
            boundaries.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: name.into(),
                    track: c::ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
        // Preflight the actual partition before persisting its workset change.
        let partition = self.plan_partition_with_authority_grants(
            &plot,
            &resident,
            std::slice::from_ref(&input.authority),
            &boundaries,
        )?;
        if self.resident.as_ref() != Some(&resident) {
            let evidence = self.session.evidence();
            let old = self
                .resident
                .as_ref()
                .ok_or("Source publication has no current resident")?;
            let checked = old.checked_plot_id.as_str().to_owned();
            let revision = evidence.body.workload_revision;
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
#[cfg(test)]
#[path = "publication/tests.rs"]
pub(crate) mod tests;
