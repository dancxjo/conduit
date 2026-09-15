use conduit_presentation::{
    ApplicationComponent, ApplicationNodeState, ApplicationView, ApplicationViewRefusal,
};

pub const MAX_DESKTOP_LINES: usize = 96;
pub const MAX_DESKTOP_LINE_BYTES: usize = 320;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopPresentation {
    pub revision: u32,
    pub title: String,
    pub lines: Vec<DesktopLine>,
    pub actions: Vec<DesktopAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopLine {
    pub depth: u8,
    pub role: ApplicationComponent,
    pub text: String,
    pub state: ApplicationNodeState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopAction {
    pub id: String,
    pub label: String,
    pub state: ApplicationNodeState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesktopPresenterRefusal {
    InvalidView(ApplicationViewRefusal),
    TooManyLines,
    LineTooLong,
    MissingTitle,
    BlankTransient,
    InvalidParent,
}

/// Renderer-local projection. It consumes the portable application view and owns no Tour state.
pub struct DesktopPresenter;

impl DesktopPresenter {
    pub fn project(view: &ApplicationView) -> Result<DesktopPresentation, DesktopPresenterRefusal> {
        view.validate()
            .map_err(DesktopPresenterRefusal::InvalidView)?;
        if view.nodes.len() > MAX_DESKTOP_LINES {
            return Err(DesktopPresenterRefusal::TooManyLines);
        }
        let title = view
            .nodes
            .iter()
            .find(|node| {
                matches!(
                    node.component,
                    ApplicationComponent::Heading | ApplicationComponent::Panel
                )
            })
            .map(|node| node.text.trim())
            .filter(|title| !title.is_empty())
            .ok_or(DesktopPresenterRefusal::MissingTitle)?
            .to_owned();
        let mut lines = Vec::with_capacity(view.nodes.len());
        let mut actions = Vec::with_capacity(view.actions.len());
        for (index, node) in view.nodes.iter().enumerate() {
            let depth = depth(view, index)?;
            let text = visible_text(node);
            if text.len() > MAX_DESKTOP_LINE_BYTES {
                return Err(DesktopPresenterRefusal::LineTooLong);
            }
            if is_transient_content(node.component) && text.trim().is_empty() {
                return Err(DesktopPresenterRefusal::BlankTransient);
            }
            if !text.is_empty() {
                lines.push(DesktopLine {
                    depth,
                    role: node.component,
                    text,
                    state: node.state,
                });
            }
            if let Some(action) = node.action {
                let action = view.actions.get(usize::from(action)).ok_or(
                    DesktopPresenterRefusal::InvalidView(ApplicationViewRefusal::UnknownAction),
                )?;
                actions.push(DesktopAction {
                    id: action.id.clone(),
                    label: node.text.clone(),
                    state: node.state,
                });
            }
        }
        Ok(DesktopPresentation {
            revision: view.revision,
            title,
            lines,
            actions,
        })
    }
}

fn visible_text(node: &conduit_presentation::ApplicationViewNode) -> String {
    match (node.text.trim(), node.value.trim()) {
        ("", "") => String::new(),
        ("", value) => value.to_owned(),
        (text, "") => text.to_owned(),
        (text, value) if text == value => text.to_owned(),
        (text, value) => format!("{text}: {value}"),
    }
}

fn is_transient_content(component: ApplicationComponent) -> bool {
    matches!(
        component,
        ApplicationComponent::Status
            | ApplicationComponent::SuccessStatus
            | ApplicationComponent::FailureStatus
            | ApplicationComponent::WarningStatus
            | ApplicationComponent::Summary
    )
}

fn depth(view: &ApplicationView, mut index: usize) -> Result<u8, DesktopPresenterRefusal> {
    let mut depth = 0_u8;
    while let Some(parent) = view.nodes[index].parent {
        index = usize::from(parent);
        if index >= view.nodes.len() {
            return Err(DesktopPresenterRefusal::InvalidParent);
        }
        depth = depth
            .checked_add(1)
            .ok_or(DesktopPresenterRefusal::InvalidParent)?;
    }
    Ok(depth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_presentation::{ApplicationEvent, ApplicationEventKind};
    use conduit_tour_model::{NEXT_CHAPTER_ACTION_ID, TOUR_CHAPTER_COUNT, TourApplicationPort};

    #[test]
    fn canonical_shared_tour_projects_nonblank_native_content() {
        let bytes = TourApplicationPort::canonical().apply(&[]).unwrap().view;
        let view = ApplicationView::decode(&bytes).unwrap();
        let presentation = DesktopPresenter::project(&view).unwrap();
        assert_eq!(presentation.revision, view.revision);
        assert!(presentation.title.contains("One Program"));
        assert!(
            presentation
                .lines
                .iter()
                .all(|line| !line.text.trim().is_empty())
        );
        assert!(
            presentation
                .actions
                .iter()
                .any(|action| action.id == "tour.run")
        );
        assert!(
            presentation
                .actions
                .iter()
                .any(|action| action.id == "tour.chapter.next")
        );
    }

    #[test]
    fn native_projection_visits_every_shared_chapter_without_blank_status() {
        let mut port = TourApplicationPort::canonical();
        let mut chapter_titles = std::collections::BTreeSet::new();
        for chapter in 0..TOUR_CHAPTER_COUNT {
            let bytes = port.apply(&[]).unwrap().view;
            let view = ApplicationView::decode(&bytes).unwrap();
            let presentation = DesktopPresenter::project(&view).unwrap();
            assert!(chapter_titles.insert(presentation.title.clone()));
            assert!(presentation.lines.iter().all(|line| {
                !matches!(
                    line.role,
                    ApplicationComponent::Status
                        | ApplicationComponent::SuccessStatus
                        | ApplicationComponent::FailureStatus
                        | ApplicationComponent::WarningStatus
                        | ApplicationComponent::Summary
                ) || !line.text.trim().is_empty()
            }));
            if chapter + 1 < TOUR_CHAPTER_COUNT {
                let event = ApplicationEvent {
                    revision: view.revision,
                    action: NEXT_CHAPTER_ACTION_ID.into(),
                    kind: ApplicationEventKind::Activate,
                    value: Vec::new(),
                };
                port.apply(&event.encode(&view).unwrap()).unwrap();
            }
        }
        assert_eq!(chapter_titles.len(), usize::from(TOUR_CHAPTER_COUNT));
    }
}
