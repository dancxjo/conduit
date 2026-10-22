//! Shared bounded reviewed inventory; no Host or renderer facts.
use conduit_body::{BodyState, BodyWorkset, ResidentPlot, MAX_BODY_PLOTS};
use conduit_core::{CheckedPlotId, SourceDocumentId};
use conduit_face::{
    FaceAction, FaceActionAvailability, FaceDisclosure, FaceDisclosureLevel, FaceProperty,
    FacePropertyValue, FaceRole, FaceSubject,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub const MAX_REVIEWED_BODY_PLOTS: usize = 16;
pub const MAX_REVIEWED_BODY_LABEL_BYTES: usize = 128;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedBodyPlot {
    pub label: String,
    pub source_document_id: String,
    pub checked_plot_id: String,
}
pub struct ReviewedInventoryProjection {
    pub subjects: Vec<FaceSubject>,
    pub properties: Vec<FaceProperty>,
    pub actions: Vec<FaceAction>,
    pub disclosures: Vec<FaceDisclosure>,
}
pub fn validate_reviewed_body_plots(reviewed: &[ReviewedBodyPlot]) -> Result<(), String> {
    if reviewed.len() > MAX_REVIEWED_BODY_PLOTS {
        return Err("reviewed plot inventory exceeds its finite bound".into());
    }
    let mut identities = BTreeSet::new();
    for plot in reviewed {
        if plot.label.trim().is_empty()
            || plot.label.len() > MAX_REVIEWED_BODY_LABEL_BYTES
            || plot.source_document_id.is_empty()
            || plot.checked_plot_id.is_empty()
            || !identities.insert(plot.checked_plot_id.as_str())
        {
            return Err("reviewed plot inventory identity is invalid or duplicated".into());
        }
    }
    Ok(())
}

pub fn project_reviewed_body_plots(
    reviewed: &[ReviewedBodyPlot],
    workset: &BodyWorkset,
    state: &BodyState,
    workload_revision: u64,
) -> Result<ReviewedInventoryProjection, String> {
    validate_reviewed_body_plots(reviewed)?;
    let mut projection = ReviewedInventoryProjection {
        subjects: Vec::new(),
        properties: Vec::new(),
        actions: Vec::new(),
        disclosures: Vec::new(),
    };
    for plot in reviewed {
        let resident = ResidentPlot::new(
            SourceDocumentId::from(plot.source_document_id.as_str()),
            CheckedPlotId::from(plot.checked_plot_id.as_str()),
        );
        if let Some(active) = workset
            .plots()
            .iter()
            .find(|active| active.checked_plot_id == resident.checked_plot_id)
        {
            if active != &resident {
                return Err("reviewed plot collides with an active checked identity".into());
            }
            continue;
        }
        let identity = format!("plot/{}", plot.checked_plot_id);
        projection.subjects.push(FaceSubject {
            identity: identity.clone(),
            role: FaceRole::Plot,
            name: plot.label.clone(),
        });
        projection.properties.extend([
            identity_property(&identity, "source-document-id", &plot.source_document_id),
            identity_property(&identity, "checked-plot-id", &plot.checked_plot_id),
            FaceProperty {
                subject: identity.clone(),
                name: "workload-membership".into(),
                value: FacePropertyValue::Text("available".into()),
            },
        ]);
        projection.actions.push(FaceAction {
            identity: format!(
                "action/add-plot/{}/{}",
                plot.checked_plot_id, workload_revision
            ),
            intent: "conduit.intent/add-plot@1".into(),
            target: identity.clone(),
            name: "Add to Body".into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::CurrentAction,
            availability: match state {
                BodyState::Awake { .. } => FaceActionAvailability::Unavailable {
                    reason_code: "body-awake".into(),
                    explanation: "Lull the body before changing its active plot workload.".into(),
                },
                BodyState::Lulled if workset.len() >= MAX_BODY_PLOTS => {
                    FaceActionAvailability::Unavailable {
                        reason_code: "workload-capacity".into(),
                        explanation: "The body active plot workload is at capacity.".into(),
                    }
                }
                BodyState::Lulled => FaceActionAvailability::Available,
                BodyState::Fulfilled { .. } => FaceActionAvailability::Unavailable {
                    reason_code: "body-fulfilled".into(),
                    explanation:
                        "A fulfilled Body cannot admit another plot; birth a new body instead."
                            .into(),
                },
            },
        });
        projection.disclosures.push(FaceDisclosure {
            subject: identity,
            level: FaceDisclosureLevel::Context,
        });
    }
    Ok(projection)
}

fn identity_property(subject: &str, name: &str, identity: &str) -> FaceProperty {
    FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value: FacePropertyValue::Identity(identity.into()),
    }
}
