//! Browser candidate conversion delegates to shared exact inventory meaning.
use crate::BrowserReviewedPlot;
pub(crate) use conduit_body_make::{
    project_reviewed_body_plots as project, validate_reviewed_body_plots as validate,
};
use conduit_patchbay_workbench::{PlotCandidate, MAX_FRONT_DOOR_PLOTS};
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{BodyState, BodyWorkset, ResidentPlot};
    use conduit_core::{CheckedPlotId, SourceDocumentId};

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
            FaceActionAvailability::Available
        ));

        assert!(validate(&[hello.clone(), hello]).is_err());
        assert!(validate(&vec![reviewed("plot"); MAX_FRONT_DOOR_PLOTS + 1]).is_err());
    }
}
