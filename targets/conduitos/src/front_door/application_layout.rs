//! Shared canonical bounded reading policy; native state lives in Mask.
use super::{Error, FrontDoor};
use conduit_face::{ApplicationAction, ApplicationView};
#[cfg(test)]
use conduit_face::{
    ApplicationComponent, ApplicationEventKind, ApplicationNodeState, ApplicationViewNode,
};
use conduitos_mask_core::layout::enabled_action;
pub(super) use conduitos_mask_core::layout::{ApplicationViewport, PAGE_ROWS, rows};
#[cfg(not(all(target_arch = "x86_64", target_os = "none")))]
impl FrontDoor {
    pub fn application_pages(&self) -> usize {
        self.application_view
            .as_ref()
            .map_or(0, |view| rows(view, |_, _| {}).div_ceil(PAGE_ROWS))
    }

    pub fn application_page(&self) -> usize {
        self.application_viewport.page
    }

    pub fn reset_application_navigation(&mut self) {
        self.application_viewport = ApplicationViewport::default();
    }

    pub fn application_view(&self) -> Option<&ApplicationView> {
        self.application_view.as_ref()
    }

    pub fn selected_application_action(&self) -> Option<&ApplicationAction> {
        let view = self.application_view.as_ref()?;
        let node = view.nodes.get(self.application_viewport.selected_node?)?;
        enabled_action(view, node)
    }

    pub fn navigate_application(&mut self, usage: u8, revision: u64) -> Result<bool, Error> {
        if revision != self.revision {
            return Err(Error::StaleInput);
        }
        if self.exact_details_open {
            return Ok(false);
        }
        let Some(view) = self.application_view.as_ref() else {
            return Ok(false);
        };
        view.validate().map_err(|_| Error::Face)?;
        if !matches!(usage, 75 | 78 | 81 | 82) {
            return Ok(false);
        }
        self.application_viewport
            .navigate(view, usage)
            .map_err(|_| Error::Face)?;
        self.advance()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::String, vec};

    fn door_with_actions() -> FrontDoor {
        let mut door = FrontDoor::new(
            conduit_core::HostId::from("host/layout"),
            conduit_core::BootId::from("boot/layout"),
            conduit_core::OfferGeneration(1),
            "profile/layout",
            "build/layout",
            "image/layout",
            conduit_core::SourceDocumentId::from("source/layout"),
            conduit_core::CheckedPlotId::from("checked/layout"),
            1,
            true,
        );
        let mut nodes = vec![ApplicationViewNode {
            parent: None,
            component: ApplicationComponent::Shell,
            key: "shell".into(),
            text: String::new(),
            value: String::new(),
            value_capacity: 0,
            action: None,
            state: ApplicationNodeState::Ready,
        }];
        for index in 0..12 {
            nodes.push(ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Paragraph,
                key: alloc::format!("text-{index}"),
                text: alloc::format!("Description {index}"),
                value: String::new(),
                value_capacity: 0,
                action: None,
                state: ApplicationNodeState::Ready,
            });
        }
        let mut actions = vec![];
        for (index, state) in [
            ApplicationNodeState::Ready,
            ApplicationNodeState::Unavailable,
            ApplicationNodeState::Ready,
        ]
        .into_iter()
        .enumerate()
        {
            nodes.push(ApplicationViewNode {
                parent: Some(0),
                component: ApplicationComponent::Button,
                key: alloc::format!("button-{index}"),
                text: alloc::format!("Action {index}"),
                value: String::new(),
                value_capacity: 0,
                action: (state == ApplicationNodeState::Ready).then_some(index as u8),
                state,
            });
            actions.push(ApplicationAction {
                id: alloc::format!("action-{index}"),
                event: ApplicationEventKind::Activate,
            });
        }
        let view = ApplicationView {
            revision: 1,
            nodes,
            actions,
        };
        view.validate().unwrap();
        door.application_view = Some(view);
        door
    }

    #[test]
    fn keyboard_navigation_reaches_actions_below_the_first_page_and_skips_unavailable() {
        let mut door = door_with_actions();
        let stale = door.revision();
        assert!(door.navigate_application(81, door.revision()).unwrap());
        assert_eq!(door.selected_application_action().unwrap().id, "action-0");
        assert!(door.application_viewport.page > 0);
        assert_eq!(door.navigate_application(81, stale), Err(Error::StaleInput));
        door.navigate_application(81, door.revision()).unwrap();
        assert_eq!(door.selected_application_action().unwrap().id, "action-2");
        door.navigate_application(82, door.revision()).unwrap();
        assert_eq!(door.selected_application_action().unwrap().id, "action-0");
        door.navigate_application(75, door.revision()).unwrap();
        assert!(door.selected_application_action().is_none());
    }

    #[test]
    fn reading_pages_are_bounded_and_reset_removes_old_focus() {
        let mut door = door_with_actions();
        for _ in 0..20 {
            door.navigate_application(78, door.revision()).unwrap();
        }
        assert_eq!(door.application_viewport.page, 2);
        for _ in 0..20 {
            door.navigate_application(75, door.revision()).unwrap();
        }
        assert_eq!(door.application_viewport.page, 0);
        door.navigate_application(81, door.revision()).unwrap();
        door.reset_application_navigation();
        assert_eq!(door.application_viewport.page, 0);
        assert!(door.selected_application_action().is_none());
    }

    #[test]
    fn pagination_preserves_all_unicode_text_and_definition_values() {
        let text = "An ordinary person reads every word of this long description, including café and 日本語, across several pages.";
        let view = ApplicationView {
            revision: 1,
            actions: vec![],
            nodes: vec![ApplicationViewNode {
                parent: None,
                component: ApplicationComponent::Definition,
                key: "meaning".into(),
                text: text.into(),
                value: "exact/body/identity".into(),
                value_capacity: 64,
                action: None,
                state: ApplicationNodeState::Ready,
            }],
        };
        let mut seen = String::new();
        let total = rows(&view, |_, row| {
            assert!(row.text.chars().count() <= LINE_CHARACTERS);
            seen.push_str(row.text);
        });
        assert!(total > 2);
        assert_eq!(
            seen.replace(' ', ""),
            alloc::format!("{text}exact/body/identity").replace(' ', "")
        );
    }

    #[test]
    fn unavailable_and_non_activation_controls_are_not_activated() {
        let mut view = ApplicationView {
            revision: 1,
            actions: vec![ApplicationAction {
                id: "body.wake".into(),
                event: ApplicationEventKind::Activate,
            }],
            nodes: vec![ApplicationViewNode {
                parent: None,
                component: ApplicationComponent::Button,
                key: "wake".into(),
                text: "Wake".into(),
                value: String::new(),
                value_capacity: 0,
                action: Some(0),
                state: ApplicationNodeState::Unavailable,
            }],
        };
        assert!(enabled_action(&view, &view.nodes[0]).is_none());
        view.nodes[0].state = ApplicationNodeState::Ready;
        assert!(enabled_action(&view, &view.nodes[0]).is_some());
        view.actions[0].event = ApplicationEventKind::Input;
        assert!(enabled_action(&view, &view.nodes[0]).is_none());
    }
}
