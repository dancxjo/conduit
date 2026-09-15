//! Deterministic packaged journey through the shared Tour application port.

use conduit_presentation::{ApplicationEvent, ApplicationNodeState, ApplicationView};
use conduit_tour_model::{
    NEXT_CHAPTER_ACTION_ID, NEXT_STAGE_ACTION_ID, OPEN_PATCHBAY_ACTION_ID, RUN_ACTION_ID,
    TOUR_CHAPTERS, TourApplicationPort, TourWorkspaceRequest,
};

use crate::{DesktopPresenter, HostedTourExecutor, HostedTourExecutorRefusal};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedTourJourney {
    pub chapters_visited: u8,
    pub exercises_completed: u8,
    pub patchbays_opened: u8,
}

pub fn run_hosted_tour_journey() -> Result<HostedTourJourney, String> {
    let mut port = TourApplicationPort::canonical();
    let mut view = current_view(&mut port)?;
    let mut report = HostedTourJourney {
        chapters_visited: 0,
        exercises_completed: 0,
        patchbays_opened: 0,
    };
    for (chapter_index, chapter) in TOUR_CHAPTERS.iter().enumerate() {
        DesktopPresenter::project(&view)
            .map_err(|error| format!("chapter presentation refused: {error:?}"))?;
        report.chapters_visited = report.chapters_visited.saturating_add(1);
        for (stage_index, _) in chapter.stages.iter().enumerate() {
            let request = activate(&mut port, &view, RUN_ACTION_ID)?;
            let (requested_chapter, requested_stage) = match request {
                Some(TourWorkspaceRequest::Run { chapter, stage }) => (chapter, stage),
                request => return Err(format!("Tour run emitted wrong request {request:?}")),
            };
            if usize::from(requested_chapter) != chapter_index
                || usize::from(requested_stage) != stage_index
            {
                return Err("Tour run request did not name the visible stage".into());
            }
            let proof = HostedTourExecutor::run(requested_chapter, requested_stage)
                .map_err(format_executor)?;
            port.complete_run(proof)
                .map_err(|error| format!("shared Tour completion refused: {error:?}"))?;
            report.exercises_completed = report.exercises_completed.saturating_add(1);
            view = current_view(&mut port)?;
            DesktopPresenter::project(&view)
                .map_err(|error| format!("result presentation refused: {error:?}"))?;
            if stage_index + 1 < chapter.stages.len() {
                activate(&mut port, &view, NEXT_STAGE_ACTION_ID)?;
                view = current_view(&mut port)?;
            }
        }
        if action_available(&view, OPEN_PATCHBAY_ACTION_ID) {
            match activate(&mut port, &view, OPEN_PATCHBAY_ACTION_ID)? {
                Some(TourWorkspaceRequest::OpenPatchbay) => {
                    report.patchbays_opened = report.patchbays_opened.saturating_add(1);
                }
                request => return Err(format!("Patchbay emitted wrong request {request:?}")),
            }
            view = current_view(&mut port)?;
            DesktopPresenter::project(&view)
                .map_err(|error| format!("Patchbay presentation refused: {error:?}"))?;
        }
        if chapter_index + 1 < TOUR_CHAPTERS.len() {
            activate(&mut port, &view, NEXT_CHAPTER_ACTION_ID)?;
            view = current_view(&mut port)?;
        }
    }
    if report.chapters_visited != TOUR_CHAPTERS.len() as u8
        || report.exercises_completed
            != TOUR_CHAPTERS
                .iter()
                .map(|chapter| chapter.stages.len() as u8)
                .sum::<u8>()
        || report.patchbays_opened == 0
    {
        return Err(format!("hosted Tour journey incomplete: {report:?}"));
    }
    Ok(report)
}

fn current_view(port: &mut TourApplicationPort) -> Result<ApplicationView, String> {
    let output = port
        .apply(&[])
        .map_err(|error| format!("Tour view request refused: {error:?}"))?;
    ApplicationView::decode(&output.view)
        .map_err(|error| format!("Tour view did not decode: {error:?}"))
}

fn activate(
    port: &mut TourApplicationPort,
    view: &ApplicationView,
    action_id: &str,
) -> Result<Option<TourWorkspaceRequest>, String> {
    let action = view
        .actions
        .iter()
        .find(|action| action.id == action_id)
        .ok_or_else(|| format!("Tour action '{action_id}' is absent"))?;
    if !action_available(view, action_id) {
        return Err(format!("Tour action '{action_id}' is unavailable"));
    }
    let event = ApplicationEvent {
        revision: view.revision,
        action: action.id.clone(),
        kind: action.event,
        value: Vec::new(),
    };
    let encoded = event
        .encode(view)
        .map_err(|error| format!("Tour action '{action_id}' did not encode: {error:?}"))?;
    port.apply(&encoded)
        .map(|output| output.request)
        .map_err(|error| format!("Tour action '{action_id}' was refused: {error:?}"))
}

fn action_available(view: &ApplicationView, action_id: &str) -> bool {
    let Some(index) = view
        .actions
        .iter()
        .position(|action| action.id == action_id)
    else {
        return false;
    };
    view.nodes.iter().any(|node| {
        node.action == u8::try_from(index).ok() && node.state == ApplicationNodeState::Ready
    })
}

fn format_executor(error: HostedTourExecutorRefusal) -> String {
    format!("hosted Tour execution refused: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_hosted_journey_visits_every_page_and_runs_every_exercise() {
        let report = run_hosted_tour_journey().unwrap();
        assert_eq!(report.chapters_visited, 7);
        assert_eq!(report.exercises_completed, 7);
        assert!(report.patchbays_opened > 0);
    }
}
