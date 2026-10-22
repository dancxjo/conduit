//! Browser composition of the accepted Body attachment, current frame, and
//! readable `BODY / SIGNS` history. This module adds no Body truth: it retains
//! the validated evidence bytes and serializes the shared model projections.

use conduit_body_make::{
    BodyEvidenceAttachment, BodyEvidenceEntrance, BodyEvidenceEntranceError, CurrentBodyFrame,
    ReadableBodyHistory, ReadableBodyHistoryError,
};
use conduit_core::{BootId, HostId, ImplementationId, PlanId, SignId};
use conduit_face::Face;
use conduit_patchbay_workbench::{
    prepare_renderer_execution, PlotCandidate, RendererAdapterIdentity, RendererAdapterKind,
};
use patchbay_application::PatchbayNavigationProjection;

use crate::{
    BrowserBodyWorkbench, BrowserBodyWorkbenchEntrance, BrowserReviewedPlot, RendererSnapshot,
    SnapshotError,
};

pub const BODY_WORKBENCH_SCHEMA: &str = "conduit.patchbay/browser-body-workbench@1";

#[derive(Debug)]
pub enum BodyWorkbenchError {
    Entrance(BodyEvidenceEntranceError),
    History(ReadableBodyHistoryError),
    Encode(serde_json::Error),
    IdentityMismatch,
    Snapshot(SnapshotError),
    Projection(String),
}

pub fn body_workbench_snapshot(
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    body_workbench_snapshot_with_reviewed(evidence_revision, encoded_evidence, entrance, &[])
}

pub fn body_workbench_snapshot_with_plots(
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
    plots: &[PlotCandidate],
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    let reviewed = crate::body_workbench_inventory::from_candidates(plots)
        .map_err(BodyWorkbenchError::Projection)?;
    body_workbench_snapshot_with_reviewed(evidence_revision, encoded_evidence, entrance, &reviewed)
}

pub(crate) fn body_workbench_snapshot_with_reviewed(
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
    reviewed_plots: &[BrowserReviewedPlot],
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    let attachment =
        BodyEvidenceAttachment::open_serialized(encoded_evidence, model_entrance(&entrance))
            .map_err(BodyWorkbenchError::Entrance)?;
    let face = workbench_face(evidence_revision, &attachment, reviewed_plots)?;
    let execution = prepare_renderer_execution(
        face,
        RendererAdapterKind::HtmlDomSvg,
        RendererAdapterIdentity {
            host_id: HostId::from("patchbay-html/body-workbench"),
            boot_id: BootId::from("patchbay-html/body-workbench/boot"),
            target_subject: "patchbay-html/body-workbench/document".into(),
        },
        SignId::from("patchbay-html/body-workbench/prepared"),
    )
    .map_err(|error| BodyWorkbenchError::Projection(error.to_string()))?;
    let mut snapshot =
        RendererSnapshot::from_execution(execution).map_err(BodyWorkbenchError::Snapshot)?;
    let navigation = PatchbayNavigationProjection::for_embodied(&snapshot.face)
        .map_err(BodyWorkbenchError::Projection)?;
    snapshot
        .attach_navigation(navigation)
        .map_err(BodyWorkbenchError::Snapshot)?;
    attach_body_workbench_with_reviewed(
        snapshot,
        evidence_revision,
        encoded_evidence,
        entrance,
        reviewed_plots,
    )
}

impl core::fmt::Display for BodyWorkbenchError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "cannot attach Patchbay Body workbench: {self:?}")
    }
}

impl std::error::Error for BodyWorkbenchError {}

pub fn attach_body_workbench(
    snapshot: RendererSnapshot,
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    attach_body_workbench_with_reviewed(
        snapshot,
        evidence_revision,
        encoded_evidence,
        entrance,
        &[],
    )
}

fn attach_body_workbench_with_reviewed(
    mut snapshot: RendererSnapshot,
    evidence_revision: u64,
    encoded_evidence: &[u8],
    entrance: BrowserBodyWorkbenchEntrance,
    reviewed_plots: &[BrowserReviewedPlot],
) -> Result<RendererSnapshot, BodyWorkbenchError> {
    let model_entrance = model_entrance(&entrance);
    let attachment = BodyEvidenceAttachment::open_serialized(encoded_evidence, model_entrance)
        .map_err(BodyWorkbenchError::Entrance)?;
    let current = CurrentBodyFrame::from_attachment(evidence_revision, &attachment);
    let history = ReadableBodyHistory::from_attachment(evidence_revision, &attachment)
        .map_err(BodyWorkbenchError::History)?;
    let workbench = BrowserBodyWorkbench {
        schema: BODY_WORKBENCH_SCHEMA.into(),
        evidence_revision,
        encoded_evidence: encoded_evidence.to_vec(),
        entrance,
        body_id: attachment.evidence().body_id.as_str().into(),
        reviewed_plots: reviewed_plots.to_vec(),
        current: serde_json::to_value(current).map_err(BodyWorkbenchError::Encode)?,
        history: serde_json::to_value(history).map_err(BodyWorkbenchError::Encode)?,
    };
    validate_body_workbench(&workbench, &snapshot.face)?;
    snapshot
        .attach_body_workbench(workbench)
        .map_err(BodyWorkbenchError::Snapshot)?;
    Ok(snapshot)
}

pub(crate) fn validate_body_workbench(
    workbench: &BrowserBodyWorkbench,
    face: &Face,
) -> Result<(), BodyWorkbenchError> {
    if workbench.schema != BODY_WORKBENCH_SCHEMA || workbench.evidence_revision == 0 {
        return Err(BodyWorkbenchError::IdentityMismatch);
    }
    let attachment = BodyEvidenceAttachment::open_serialized(
        &workbench.encoded_evidence,
        model_entrance(&workbench.entrance),
    )
    .map_err(BodyWorkbenchError::Entrance)?;
    crate::body_workbench_inventory::validate(&workbench.reviewed_plots)
        .map_err(BodyWorkbenchError::Projection)?;
    let inventory = crate::body_workbench_inventory::project(
        &workbench.reviewed_plots,
        &attachment.evidence().body.workset,
        &attachment.evidence().body.state,
        attachment.evidence().body.workload_revision,
    )
    .map_err(BodyWorkbenchError::Projection)?;
    let presented_add_actions = face
        .actions
        .iter()
        .filter(|action| action.intent == "conduit.intent/add-plot@1")
        .cloned()
        .collect::<Vec<_>>();
    let inventory_matches = presented_add_actions == inventory.actions
        && inventory
            .subjects
            .iter()
            .all(|subject| face.subjects.contains(subject));
    let body_id = attachment.evidence().body_id.as_str();
    let body_matches = face
        .basis
        .body_id
        .as_ref()
        .is_some_and(|identity| identity.as_str() == body_id);
    let basis_plot = face
        .basis
        .source_document_id
        .as_ref()
        .zip(face.basis.checked_plot_id.as_ref());
    let plot_matches = basis_plot.is_none_or(|(source, checked)| {
        attachment
            .evidence()
            .body
            .workset
            .contains(&conduit_body::ResidentPlot::new(
                source.clone(),
                checked.clone(),
            ))
    });
    let expected_current = serde_json::to_value(CurrentBodyFrame::from_attachment(
        workbench.evidence_revision,
        &attachment,
    ))
    .map_err(BodyWorkbenchError::Encode)?;
    let expected_history = serde_json::to_value(
        ReadableBodyHistory::from_attachment(workbench.evidence_revision, &attachment)
            .map_err(BodyWorkbenchError::History)?,
    )
    .map_err(BodyWorkbenchError::Encode)?;
    if workbench.body_id != body_id
        || !body_matches
        || !plot_matches
        || !inventory_matches
        || workbench.current != expected_current
        || workbench.history != expected_history
    {
        return Err(BodyWorkbenchError::IdentityMismatch);
    }
    Ok(())
}

pub(crate) fn model_entrance(entrance: &BrowserBodyWorkbenchEntrance) -> BodyEvidenceEntrance {
    match entrance {
        BrowserBodyWorkbenchEntrance::Hosted {
            plan_id,
            implementation_id,
        } => BodyEvidenceEntrance::Hosted {
            plan_id: PlanId::from(plan_id.as_str()),
            implementation_id: ImplementationId::from(implementation_id.as_str()),
        },
        BrowserBodyWorkbenchEntrance::ExternalReader => BodyEvidenceEntrance::ExternalReader,
    }
}

fn workbench_face(
    revision: u64,
    attachment: &BodyEvidenceAttachment,
    reviewed_plots: &[BrowserReviewedPlot],
) -> Result<Face, BodyWorkbenchError> {
    conduit_body_make::project_body_workbench_face(revision, attachment, reviewed_plots)
        .map_err(BodyWorkbenchError::Projection)
}

#[cfg(test)]
mod tests;
