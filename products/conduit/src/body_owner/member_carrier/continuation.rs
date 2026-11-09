//! Owner-derived signed offline recipe, selected only by local File adoption.
//! The private original issuer signs a typed Body transcript; no raw signing
//! entrance, copied key or candidate full-Wake projection is accepted.
use super::*;
#[cfg(target_os = "linux")]
impl Owner {
    pub(crate) fn selected_continuation_artifact(
        &self,
        selected: &crate::durable_host::source_continuation::PreparedOutput,
    ) -> Result<[u8; conduit_body::continuation_artifact::PREFIX_BYTES], String> {
        use conduit_body::{
            continuation_artifact as artifact, ContinuationFullScope, ContinuationPlotPin,
        };
        use sha2::{Digest, Sha256};
        let selection = selected.selection();
        let recipe = selected.recipe();
        let member = selected.member();
        self.require_current_native_member(member)?;
        self.require_observed_member_offer(&member.host_advertisement)?;
        if selection.host_id != member.credential.host_id.as_str()
            || recipe.configuration_id != member.host_advertisement.profile.as_str()
            || self.session.evidence().membership.revision.0 != selection.membership_generation
        {
            return Err(
                "selected continuation differs from actual accepted member/configuration/revision"
                    .into(),
            );
        }
        let retained = self
            .selected_file_workset
            .as_ref()
            .ok_or("checked selected Source custody absent")?;
        retained.verify(&self.host, &self.session.evidence().body.workset)?;
        let source_digest: [u8; 32] =
            Sha256::digest(retained.checked.source.source.as_bytes()).into();
        if recipe.source_digest != source_digest {
            return Err("sealed recipe Source differs from original retained capture".into());
        }
        recipe
            .checked_digest()
            .map_err(|e| format!("sealed continuation recipe: {e:?}"))?;
        let realization = self
            .session
            .realization()
            .ok_or("actual complete Body proposal required")?;
        realization
            .plan
            .validate_for(&realization.wake)
            .map_err(|e| format!("retained Body Plan: {e:?}"))?;
        if !realization.plan.mask_topologies.is_empty()
            || realization.plan.body_time_requirement.is_some()
        {
            return Err(
                "continuation scope currently supports exact plain Plot worksets only".into(),
            );
        }
        let mut pins = Vec::with_capacity(realization.plan.plots.len());
        let mut local = 0usize;
        for plot in &realization.plan.plots {
            let local_fragments = plot
                .plan
                .fragments
                .iter()
                .filter(|f| f.host_id == member.host_advertisement.host_id)
                .count();
            if local_fragments != 0 && local_fragments != plot.plan.fragments.len() {
                return Err("continuation cannot replace a partially local Plot".into());
            }
            let is_local = local_fragments != 0;
            if is_local {
                local += 1;
                let pin = recipe
                    .entries
                    .iter()
                    .find(|e| {
                        e.source_document == plot.plot.source_document_id.as_str()
                            && e.checked_plot == plot.plot.checked_plot_id.as_str()
                    })
                    .ok_or("actual local Plot absent from sealed Make recipe")?;
                if !retained.checked.plots.iter().any(|p| {
                    p.expanded.name == pin.name
                        && p.expanded.source_document_id.as_str() == pin.source_document
                        && p.expanded.checked_plot_id.as_str() == pin.checked_plot
                        && p.expanded.expanded_plot_id.as_str() == pin.expanded_plot
                }) {
                    return Err(
                        "Make entry differs from exact independently checked retained Source"
                            .into(),
                    );
                }
            }
            pins.push(ContinuationPlotPin {
                source_document: plot.plot.source_document_id.as_str(),
                checked_plot: plot.plot.checked_plot_id.as_str(),
                plan_id: plot.plan.plan_id.as_str(),
                local: is_local,
            });
        }
        if local != recipe.entries.len() || local == 0 {
            return Err("Make recipe is not the complete actual local subset".into());
        }
        let scope = ContinuationFullScope {
            wake_id: realization.wake.wake_id.as_str(),
            wake_sequence: realization.wake.wake_sequence,
            workload_revision: realization.plan.workload_revision,
            base_body_plan: realization.plan.plan_id.as_str(),
            plots: &pins,
        };
        let body = self.session.evidence().body_id.as_str();
        let scope_digest = conduit_body::continuation_scope_digest(body, &scope)
            .map_err(|e| format!("full retained Wake scope: {e:?}"))?;
        let peer = self.member_carrier_peer()?;
        let intent = conduit_body::ContinuationIntent {
            schema: conduit_body::CONTINUATION_INTENT_SCHEMA,
            body_id: body,
            part_id: member.credential.part_id.as_str(),
            host_id: member.credential.host_id.as_str(),
            credential_id: member.credential.credential_id.as_str(),
            membership_generation: self.session.evidence().membership.revision.0,
            intent_generation: selection.intent_generation,
            issuer_host: peer.host_id.as_str(),
            issuer_key: peer.verifying_key,
            source_digest,
            workset_digest: recipe.workset_digest,
            authority: conduit_body::ContinuationAuthority::LocalWorksetReplacement,
            scope_digest,
            image_id: &recipe.image_id,
            configuration_id: &recipe.configuration_id,
            ceilings: selection.ceilings,
        };
        // This key is the existing original owner issuer, never supplied by the
        // selection, an incoming member or a serialized artifact.
        let signature = self
            .member_carrier
            .as_ref()
            .and_then(|c| c.key.as_ref())
            .ok_or("original owner issuer absent")?
            .sign(&intent.signing_transcript());
        let mut plot_slots = [None; conduit_body::MAX_BODY_PLOTS];
        for (slot, pin) in plot_slots.iter_mut().zip(&pins) {
            *slot = Some(artifact::PlotPin {
                source_document: pin.source_document,
                checked_plot: pin.checked_plot,
                plan_id: pin.plan_id,
                local: pin.local,
            });
        }
        let document = artifact::Artifact {
            schema: artifact::SCHEMA,
            intent,
            signature: [
                signature[..32].try_into().unwrap(),
                signature[32..].try_into().unwrap(),
            ],
            wake_id: scope.wake_id,
            wake_sequence: scope.wake_sequence,
            workload_revision: scope.workload_revision,
            base_body_plan: scope.base_body_plan,
            plot_count: u8::try_from(pins.len()).map_err(|_| "full Wake bound")?,
            plots: plot_slots,
        };
        let mut bytes = [0xff; artifact::PREFIX_BYTES];
        artifact::encode(&document, &mut bytes)
            .map_err(|e| format!("finite installed continuation artifact: {e:?}"))?;
        Ok(bytes)
    }
}
