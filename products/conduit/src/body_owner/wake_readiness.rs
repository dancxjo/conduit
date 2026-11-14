//! Execution availability of generic owner Wake, separate from lifecycle permission.
//! Checked input requirements are planning evidence, never adapter possession.
use super::Owner;
use conduit_body::{Body, BodyState};
use conduit_face::{Face, FaceActionAvailability};
use conduit_plot::ExpandedAuthoringPlot;
use sha2::{Digest, Sha256};

// The inline ledger must cover every admitted resident Plot.
const _: () = assert!(conduit_body::MAX_BODY_PLOTS <= u16::BITS as usize);

#[derive(Default)]
pub(super) struct WakeIngressRequirements {
    basis: Option<[u8; 32]>,
    observed: u16,
    required: u16,
}
impl WakeIngressRequirements {
    /// Observe only a checked member of this exact workset. Partial observations
    /// remain Unknown; no Sources, names or copied Host labels select an adapter.
    pub(super) fn observe(
        &mut self,
        body: &Body,
        plot: &ExpandedAuthoringPlot,
    ) -> Result<(), String> {
        let index = body
            .workset
            .plots()
            .iter()
            .position(|p| {
                p.source_document_id == plot.expanded.source_document_id
                    && p.checked_plot_id == plot.expanded.checked_plot_id
            })
            .ok_or("Wake readiness Source differs from exact retained workset")?;
        if body.workset.plots().len() > conduit_body::MAX_BODY_PLOTS {
            return Err("Wake readiness workset exceeds finite bitmap".into());
        }
        let basis = basis(body);
        if self.basis != Some(basis) {
            *self = Self {
                basis: Some(basis),
                ..Self::default()
            };
        }
        let bit = 1u16 << index;
        self.observed |= bit;
        if super::continuing::requires_selected_input_adapter(plot) {
            self.required |= bit;
        } else {
            self.required &= !bit;
        }
        Ok(())
    }
    fn refusal(
        &self,
        body: &Body,
        mut selected: impl FnMut(&conduit_body::ResidentPlot) -> bool,
    ) -> Option<&'static str> {
        if body.state != BodyState::Lulled {
            return None;
        }
        let count = body.workset.plots().len();
        if count > conduit_body::MAX_BODY_PLOTS {
            return Some("owner-wake-input-readiness-unchecked");
        }
        let complete = if count == u16::BITS as usize {
            u16::MAX
        } else {
            (1u16 << count) - 1
        };
        if self.basis != Some(basis(body)) || self.observed != complete {
            return Some("owner-wake-input-readiness-unchecked");
        }
        body.workset
            .plots()
            .iter()
            .enumerate()
            .any(|(index, plot)| self.required & (1u16 << index) != 0 && !selected(plot))
            .then_some("service-input-adapter-required")
    }
}
fn basis(body: &Body) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(body.workload_revision.to_le_bytes());
    for text in core::iter::once(body.body_id.as_str()).chain(
        body.workset
            .plots()
            .iter()
            .flat_map(|p| [p.source_document_id.as_str(), p.checked_plot_id.as_str()]),
    ) {
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    hash.finalize().into()
}

impl Owner {
    pub(crate) fn local_face_snapshot(&self) -> Result<Face, String> {
        self.with_wake_input_readiness(self.local_face_snapshot_without_wake_input_readiness()?)
    }
    /// Called only for the generic service Start branch. The ordinary notebook,
    /// thermostat and selected Todo adapters retain their own admission policy.
    /// A selected remote Moon adapter may satisfy its exact remote partition;
    /// this never invents an equivalent local owner input or requests a sample.
    pub(super) fn with_wake_input_readiness(&self, face: Face) -> Result<Face, String> {
        if self.has_open_notebook()
            || self.is_thermostat()
            || self.has_verified_todo()
            || self.host.is_playing()
            || self.session.realization().is_some()
        {
            return Ok(face);
        }
        let body = &self.session.evidence().body;
        let root = self.host.advertisement();
        let refusal = self.wake_ingress_requirements.refusal(body, |plot| {
            self.local_fore_adapters.as_ref().is_some_and(|set| {
                set.contains_current_boundaries(
                    root,
                    plot,
                    core::iter::once(root).chain(
                        self.current_host_offers
                            .current_hosts(self.session.evidence())
                            .filter(|ad| ad.host_id != root.host_id),
                    ),
                )
            })
        });
        project(face, refusal)
    }
}
fn project(mut face: Face, refusal: Option<&'static str>) -> Result<Face, String> {
    let Some(code) = refusal else {
        return Ok(face);
    };
    let mut changed = false;
    for action in &mut face.actions {
        if action.intent == "conduit.intent/wake@1" && action.availability.is_available() {
            action.availability=FaceActionAvailability::Unavailable{reason_code:code.into(), explanation:
                if code=="service-input-adapter-required" {
                    "This workset needs its explicitly selected input adapters before the owner can start it."
                } else { "The owner has not checked the complete workset's input requirements for this Source." }.into()};
            changed = true;
        }
    }
    if !changed {
        return Ok(face);
    }
    Face::new_with_semantics_and_temporal(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
        face.temporal_references,
        face.temporal_facts,
    )
    .and_then(|v| v.with_composition(face.composition))
    .and_then(|v| v.with_interaction_context(face.interaction_context))
    .map_err(|e| format!("Wake readiness Face refused:{e:?}"))
}
#[cfg(test)]
#[path = "wake_readiness/tests.rs"]
mod tests;
