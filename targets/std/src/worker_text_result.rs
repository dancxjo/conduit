//! Finite pending text-presentation evidence, without Body or Line authority.
//!
//! A worker driver obtains this from its actual pending kernel Host Call. It
//! must leave that call pending until authenticated delivery succeeds. The
//! continuing owner authenticates execution, current child lifetime and time
//! provenance, and owns final Face projection/removal. No role is assigned here.
use conduit_core::{
    ActivePlayIdentity, BootId, CheckedPlotId, HostAdvertisement, HostId, OfferGeneration,
    PlacementId, PlanFragment, PlannedGear, SourceDocumentId,
};
use conduit_face::{FaceContributionBasis, FaceFragment, FaceRole, FaceSubject, FaceText};
use conduit_kernel::scheduler::HostCallRequest;

pub const MAXIMUM_WORKER_TEXT_BYTES: usize = 256;
pub const MAXIMUM_WORKER_LABEL_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerTextResult {
    host_id: HostId,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    source_document_id: SourceDocumentId,
    placement_id: PlacementId,
    source_node: u16,
    source_request: u32,
    fragment: FaceFragment,
}
pub(crate) struct WorkerTextBasis {
    source_document_id: SourceDocumentId,
    checked_plot_id: CheckedPlotId,
    offer_generation: OfferGeneration,
    play: ActivePlayIdentity,
}
impl WorkerTextBasis {
    pub(crate) fn new(plan: &PlanFragment, play: ActivePlayIdentity) -> Self {
        Self {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            offer_generation: plan.offer_generation,
            play,
        }
    }
}
impl WorkerTextResult {
    pub(crate) fn from_pending(
        host: &HostAdvertisement,
        basis: &WorkerTextBasis,
        placement: &PlannedGear,
        request: HostCallRequest,
        input: &[u8],
    ) -> Result<Self, String> {
        let play = &basis.play;
        if basis.offer_generation != host.offer_generation
            || play.host_id != host.host_id
            || play.boot_id != host.boot_id
        {
            return Err("worker text result has a foreign or stale execution basis".into());
        }
        if placement.kind_id.as_str() != "face/text" {
            return Err("worker text result is not a planned face/text presentation".into());
        }
        let label = conduit_semantic_catalog::face_data_label(&placement.configuration)?;
        if label.is_empty()
            || label.len() > MAXIMUM_WORKER_LABEL_BYTES
            || input.is_empty()
            || input.len() > MAXIMUM_WORKER_TEXT_BYTES
        {
            return Err("worker text result exceeds its finite text profile".into());
        }
        let text = core::str::from_utf8(input).map_err(|_| "worker text result is not UTF-8")?;
        let subject = "worker/text-result".to_string();
        let fragment = FaceFragment {
            basis: FaceContributionBasis {
                checked_plot_id: basis.checked_plot_id.clone(),
                plan_id: play.plan_id.clone(),
                active_play_id: play.active_play_id.clone(),
                required_interaction_context: None,
            },
            subjects: vec![FaceSubject {
                identity: subject.clone(),
                role: FaceRole::Info,
                name: label.to_owned(),
            }],
            text: vec![FaceText {
                subject,
                text: text.to_owned(),
            }],
            relationships: vec![],
            composition: vec![],
            properties: vec![],
            actions: vec![],
            disclosures: vec![],
            temporal_references: vec![],
            temporal_facts: vec![],
        };
        fragment
            .validate_bounds()
            .map_err(|e| format!("worker text fragment refused: {e:?}"))?;
        Ok(Self {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            source_document_id: basis.source_document_id.clone(),
            placement_id: placement.placement_id.clone(),
            source_node: request.node.0,
            source_request: request.request.0,
            fragment,
        })
    }
    pub fn fragment(&self) -> &FaceFragment {
        &self.fragment
    }
}
