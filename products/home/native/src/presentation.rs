use conduit_home_model::{HomeDestination, HomeModel, HomeView};
use conduit_presentation::{ApplicationComponent, ApplicationNodeState, ApplicationView};

pub const MAX_NATIVE_LINES: usize = 64;
pub const MAX_NATIVE_LINE_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeHomePresentation {
    pub revision: u32,
    pub view: HomeView,
    pub selected: usize,
    pub title: String,
    pub lines: Vec<NativeHomeLine>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeHomeLine {
    pub key: String,
    pub text: String,
    pub role: ApplicationComponent,
    pub state: ApplicationNodeState,
    pub selected: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeHomeRefusal {
    InvalidPortableView,
    MissingTitle,
    TooManyLines,
    LineTooLong,
}

impl NativeHomePresentation {
    pub fn project(
        model: &HomeModel,
        revision: u32,
        installed_forms: &[&str],
    ) -> Result<Self, NativeHomeRefusal> {
        let view = model
            .presentation(revision, installed_forms)
            .lower()
            .map_err(|_| NativeHomeRefusal::InvalidPortableView)?;
        Self::from_view(model, view)
    }

    fn from_view(model: &HomeModel, view: ApplicationView) -> Result<Self, NativeHomeRefusal> {
        view.validate()
            .map_err(|_| NativeHomeRefusal::InvalidPortableView)?;
        if view.nodes.len() > MAX_NATIVE_LINES {
            return Err(NativeHomeRefusal::TooManyLines);
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
            .filter(|text| !text.is_empty())
            .ok_or(NativeHomeRefusal::MissingTitle)?
            .to_owned();
        let selected_action = match model.view() {
            HomeView::Launcher => Some(model.selected_index()),
            HomeView::Forms => Some(model.selected_form_index()),
            _ => None,
        };
        let mut action_index = 0_usize;
        let mut lines = Vec::with_capacity(view.nodes.len());
        for node in &view.nodes {
            let text = visible_text(&node.text, &node.value);
            if text.is_empty()
                || matches!(
                    node.component,
                    ApplicationComponent::Heading | ApplicationComponent::Panel
                )
            {
                continue;
            }
            if text.len() > MAX_NATIVE_LINE_BYTES {
                return Err(NativeHomeRefusal::LineTooLong);
            }
            let selected = if node.action.is_some() {
                let selected = selected_action == Some(action_index);
                action_index += 1;
                selected
            } else {
                false
            };
            lines.push(NativeHomeLine {
                key: node.key.clone(),
                text,
                role: node.component,
                state: node.state,
                selected,
            });
        }
        Ok(Self {
            revision: view.revision,
            view: model.view(),
            selected: selected_action.unwrap_or(0),
            title,
            lines,
        })
    }

    pub fn selected_destination(&self) -> Option<HomeDestination> {
        (self.view == HomeView::Launcher).then(|| HomeDestination::ALL[self.selected])
    }
}

fn visible_text(text: &str, value: &str) -> String {
    match (text.trim(), value.trim()) {
        ("", "") => String::new(),
        ("", value) => value.to_owned(),
        (text, "") => text.to_owned(),
        (text, value) if text == value => text.to_owned(),
        (text, value) => format!("{text}: {value}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_home_model::{HOME_ITEM_COUNT, HomeEvent};

    const FORMS: [&str; 3] = ["Hello", "Clock", "Count"];

    #[test]
    fn every_home_view_projects_from_the_portable_contract() {
        let mut model = HomeModel::new();
        let home = NativeHomePresentation::project(&model, 1, &FORMS).unwrap();
        assert_eq!(home.lines.len(), HOME_ITEM_COUNT);
        assert_eq!(home.selected_destination(), Some(HomeDestination::Tour));

        model.submit_text("forms", &FORMS);
        let forms = NativeHomePresentation::project(&model, 2, &FORMS).unwrap();
        assert_eq!(forms.lines.len(), FORMS.len() + 1);
        assert!(
            FORMS
                .iter()
                .all(|form| forms.lines.iter().any(|line| line.text == *form))
        );

        model.submit_text("open prompt", &FORMS);
        model.accept(HomeEvent::Text("help"), &FORMS);
        let prompt = NativeHomePresentation::project(&model, 3, &FORMS).unwrap();
        assert!(prompt.lines.iter().any(|line| line.key == "command"));
        assert!(prompt.lines.iter().any(|line| line.key == "command-result"));
    }

    #[test]
    fn selection_is_host_projection_not_a_second_state_machine() {
        let mut model = HomeModel::new();
        model.accept(HomeEvent::Next, &FORMS);
        let view = NativeHomePresentation::project(&model, 9, &FORMS).unwrap();
        assert_eq!(view.selected_destination(), Some(HomeDestination::Patchbay));
        assert_eq!(view.lines.iter().filter(|line| line.selected).count(), 1);
    }
}
