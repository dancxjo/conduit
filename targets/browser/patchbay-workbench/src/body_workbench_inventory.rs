//! Bounded reviewed-Plot inventory projected beside one attached Body.

use crate::BrowserReviewedPlot;
use conduit_body::{BodyState, BodyWorkset, ResidentPlot, MAX_BODY_PLOTS};
use conduit_core::{CheckedPlotId, SourceDocumentId};
use conduit_patchbay_workbench::{PlotCandidate, MAX_FRONT_DOOR_PLOTS};
use conduit_presentation::{
    PresentationAction, PresentationActionAvailability, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationProperty, PresentationPropertyValue, PresentationRole,
    PresentationSubject,
};
use std::collections::BTreeSet;

pub(crate) struct ReviewedInventoryProjection {
    pub subjects: Vec<PresentationSubject>,
    pub properties: Vec<PresentationProperty>,
    pub actions: Vec<PresentationAction>,
    pub disclosures: Vec<PresentationDisclosure>,
}

pub(crate) fn from_candidates(
    candidates: &[PlotCandidate],
) -> Result<Vec<BrowserReviewedPlot>, String> {
    if candidates.len() > MAX_FRONT_DOOR_PLOTS {
        return Err("reviewed plot inventory exceeds its finite bound".into());
    }
    let reviewed = candidates
        .iter()
        .map(|candidate| BrowserReviewedPlot {
            label: candidate.label.clone(),
            source_document_id: candidate.source_document_id.as_str().into(),
            checked_plot_id: candidate.checked_plot_id.as_str().into(),
        })
        .collect::<Vec<_>>();
    validate(&reviewed)?;
    Ok(reviewed)
}

pub(crate) fn validate(reviewed: &[BrowserReviewedPlot]) -> Result<(), String> {
    if reviewed.len() > MAX_FRONT_DOOR_PLOTS {
        return Err("reviewed plot inventory exceeds its finite bound".into());
    }
    let mut identities = BTreeSet::new();
    for plot in reviewed {
        if plot.label.trim().is_empty()
            || plot.label.len() > crate::MAX_PLOT_LABEL_BYTES
            || plot.source_document_id.is_empty()
            || plot.checked_plot_id.is_empty()
            || !identities.insert(plot.checked_plot_id.as_str())
        {
            return Err("reviewed plot inventory identity is invalid or duplicated".into());
        }
    }
    Ok(())
}

pub(crate) fn project(
    reviewed: &[BrowserReviewedPlot],
    workset: &BodyWorkset,
    state: &BodyState,
    workload_revision: u64,
) -> Result<ReviewedInventoryProjection, String> {
    validate(reviewed)?;
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
        projection.subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Plot,
            name: plot.label.clone(),
        });
        projection.properties.extend([
            identity_property(&identity, "source-document-id", &plot.source_document_id),
            identity_property(&identity, "checked-plot-id", &plot.checked_plot_id),
            PresentationProperty {
                subject: identity.clone(),
                name: "workload-membership".into(),
                value: PresentationPropertyValue::Text("available".into()),
            },
        ]);
        projection.actions.push(PresentationAction {
            identity: format!(
                "action/add-plot/{}/{}",
                plot.checked_plot_id, workload_revision
            ),
            intent: "conduit.intent/add-plot@1".into(),
            target: identity.clone(),
            name: "Add to Body".into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: match state {
                BodyState::Awake { .. } => PresentationActionAvailability::Unavailable {
                    reason_code: "body-awake".into(),
                    explanation: "Lull the body before changing its active plot workload.".into(),
                },
                BodyState::Lulled if workset.len() >= MAX_BODY_PLOTS => {
                    PresentationActionAvailability::Unavailable {
                        reason_code: "workload-capacity".into(),
                        explanation: "The body active plot workload is at capacity.".into(),
                    }
                }
                BodyState::Lulled => PresentationActionAvailability::Available,
                BodyState::Fulfilled { .. } => PresentationActionAvailability::Unavailable {
                    reason_code: "body-fulfilled".into(),
                    explanation:
                        "A fulfilled Body cannot admit another plot; birth a new body instead."
                            .into(),
                },
            },
        });
        projection.disclosures.push(PresentationDisclosure {
            subject: identity,
            level: PresentationDisclosureLevel::Context,
        });
    }
    Ok(projection)
}

fn identity_property(subject: &str, name: &str, identity: &str) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value: PresentationPropertyValue::Identity(identity.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reviewed(name: &str) -> BrowserReviewedPlot {
        BrowserReviewedPlot {
            label: name.into(),
            source_document_id: format!("source/{name}"),
            checked_plot_id: format!("checked/{name}"),
        }
    }

    #[test]
    fn inventory_is_bounded_unique_and_excludes_exact_active_plots() {
        let hello = reviewed("hello");
        let workset = BodyWorkset::from_plots([ResidentPlot::new(
            SourceDocumentId::from(hello.source_document_id.as_str()),
            CheckedPlotId::from(hello.checked_plot_id.as_str()),
        )])
        .unwrap();
        let projected = project(
            &[hello.clone(), reviewed("clock")],
            &workset,
            &BodyState::Lulled,
            0,
        )
        .unwrap();
        assert_eq!(projected.subjects.len(), 1);
        assert_eq!(projected.subjects[0].name, "clock");
        assert_eq!(projected.actions.len(), 1);
        assert_eq!(projected.actions[0].intent, "conduit.intent/add-plot@1");
        assert!(matches!(
            projected.actions[0].availability,
            PresentationActionAvailability::Available
        ));

        assert!(validate(&[hello.clone(), hello]).is_err());
        assert!(validate(&vec![reviewed("plot"); MAX_FRONT_DOOR_PLOTS + 1]).is_err());
    }
}
