//! Native consumption of the shared truthful Patchbay entrance state.

use super::PatchbayApplication;
use crate::front_door_follow::{exact_current_follow, NativeFollowRefusal};
use conduit_core::SignId;
use conduit_presentation::{NavigationOperation, NavigationState, Presentation};
use patchbay_model::{
    PatchbayNavigationProjection, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
    ZeroBodyFrontDoor,
};

pub(super) struct NativeFrontDoorPresentation {
    pub presentation: Presentation,
    pub navigation: PatchbayNavigationProjection,
    navigation_state: NavigationState,
}

impl NativeFrontDoorPresentation {
    pub(super) fn new(
        presentation: Presentation,
        navigation: PatchbayNavigationProjection,
    ) -> Result<Self, String> {
        let navigation_state = NavigationState::new(
            &navigation.navigation,
            navigation.cursor.clone(),
            conduit_presentation::MAX_NAVIGATION_HISTORY,
        )
        .map_err(|error| format!("front-door navigation state: {error:?}"))?;
        Ok(Self {
            presentation,
            navigation,
            navigation_state,
        })
    }
}

impl PatchbayApplication {
    pub(super) fn initialize_front_door(&mut self) -> Result<(), String> {
        let session = ZeroBodyFrontDoor::from_model(self.model.clone())?;
        let projection = session.project()?;
        let presentation = projection.presentation;
        let navigation = projection.navigation;
        let execution = self.prepare_front_door_renderer(presentation.clone())?;
        self.zero_body_front_door = Some(session);
        self.entrance = Some(NativeFrontDoorPresentation::new(presentation, navigation)?);
        self.renderer_execution = Some(execution);
        Ok(())
    }

    pub(super) fn refresh_front_door(&mut self) -> Result<(), String> {
        let Some(session) = self.zero_body_front_door.as_ref() else {
            return Ok(());
        };
        let projection = session.project()?;
        let presentation = projection.presentation;
        let navigation = projection.navigation;
        if self.entrance.as_ref().is_some_and(|prior| {
            let prior = &prior.presentation;
            prior.revision == presentation.revision && prior.identity == presentation.identity
        }) {
            return Ok(());
        }
        let prior = &self
            .entrance
            .as_ref()
            .ok_or("native front-door Presentation is absent")?
            .presentation;
        if presentation.basis.body_id != prior.basis.body_id {
            return Err("front-door update: WrongBody".into());
        }
        if presentation.revision <= prior.revision {
            return Err("front-door update: StaleRevision".into());
        }
        let execution = self.prepare_front_door_renderer(presentation.clone())?;
        self.entrance = Some(NativeFrontDoorPresentation::new(presentation, navigation)?);
        self.renderer_execution = Some(execution);
        Ok(())
    }

    fn prepare_front_door_renderer(
        &self,
        presentation: Presentation,
    ) -> Result<RendererExecution, String> {
        RendererExecution::prepare(
            presentation,
            RendererAdapterKind::NativeWayland,
            RendererAdapterIdentity {
                host_id: self.model.advertisement().host_id.clone(),
                boot_id: self.model.advertisement().boot_id.clone(),
                target_subject: "patchbay-native/front-door/window".into(),
            },
            SignId::from("patchbay-native/front-door/prepared"),
        )
        .map_err(|error| error.to_string())
    }

    pub(super) fn select_front_door_subject(&mut self, subject: &str) -> Result<(), String> {
        match &mut self.entrance {
            Some(entrance) => {
                let mut next_navigation = entrance.navigation_state.clone();
                let cursor = next_navigation
                    .navigate(
                        &entrance.presentation,
                        &entrance.navigation.navigation,
                        entrance.presentation.revision,
                        NavigationOperation::Focus(subject.into()),
                    )
                    .map_err(|error| format!("portable front-door focus: {error:?}"))?
                    .clone();
                entrance.navigation.cursor = cursor;
                entrance.navigation_state = next_navigation;
                self.selected_follow = None;
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub(super) fn navigate_front_door(
        &mut self,
        operation: NavigationOperation,
    ) -> Result<(), String> {
        let entrance = self
            .entrance
            .as_mut()
            .ok_or("native front-door Presentation is absent")?;
        entrance.navigation.cursor = entrance
            .navigation_state
            .navigate(
                &entrance.presentation,
                &entrance.navigation.navigation,
                entrance.presentation.revision,
                operation,
            )
            .map_err(|error| format!("native front-door navigation refused: {error:?}"))?
            .clone();
        self.selected_follow = None;
        Ok(())
    }

    pub(super) fn follow_front_door(&mut self) -> Result<(), String> {
        let follow = self
            .entrance
            .as_ref()
            .ok_or("native front-door Presentation is absent")
            .and_then(|entrance| {
                exact_current_follow(
                    &entrance.navigation.navigation,
                    &entrance.navigation.cursor,
                    self.selected_follow.as_deref(),
                )
                .map(|follow| follow.identity.clone())
                .map_err(|refusal| match refusal {
                    NativeFollowRefusal::Unavailable => "FOLLOW unavailable for the current Focus",
                    NativeFollowRefusal::Ambiguous => {
                        "FOLLOW requires one exact current correlation"
                    }
                })
            });
        match follow {
            Ok(identity) => self.navigate_front_door(NavigationOperation::Follow(identity)),
            Err(refusal) => {
                self.publish_refusal(refusal);
                Ok(())
            }
        }
    }

    pub(super) fn cycle_front_door_follow(&mut self) -> Result<(), String> {
        let identities = {
            let entrance = self
                .entrance
                .as_ref()
                .ok_or("native front-door Presentation is absent")?;
            let Some(focus) = entrance.navigation.cursor.focus.as_deref() else {
                self.publish_refusal("FOLLOW unavailable for the current Focus");
                return Ok(());
            };
            entrance
                .navigation
                .navigation
                .follows
                .iter()
                .filter(|follow| follow.source_subject == focus)
                .map(|follow| follow.identity.clone())
                .collect::<Vec<_>>()
        };
        if identities.is_empty() {
            self.publish_refusal("FOLLOW unavailable for the current Focus");
            return Ok(());
        }
        let next = self
            .selected_follow
            .as_deref()
            .and_then(|selected| {
                identities
                    .iter()
                    .position(|identity| identity == selected)
                    .map(|index| (index + 1) % identities.len())
            })
            .unwrap_or(0);
        self.selected_follow = Some(identities[next].clone());
        if let Some(window) = &self.window {
            window.request_redraw();
        }
        Ok(())
    }

    pub(super) fn cycle_front_door_place(&mut self, delta: isize) -> Result<(), String> {
        let navigation = &self
            .entrance
            .as_ref()
            .ok_or("native front-door navigation is absent")?
            .navigation;
        let places = &navigation.navigation.places;
        if places.len() < 2 {
            return Ok(());
        }
        let current = places
            .iter()
            .position(|place| place.place == navigation.cursor.place)
            .ok_or("native front-door current Place is absent")?;
        let next = (current as isize + delta).rem_euclid(places.len() as isize) as usize;
        self.navigate_front_door(NavigationOperation::Enter(places[next].place))
    }

    pub(super) fn cycle_front_door_aspect(&mut self, delta: isize) -> Result<(), String> {
        let navigation = &self
            .entrance
            .as_ref()
            .ok_or("native front-door navigation is absent")?
            .navigation;
        let aspects = &navigation
            .navigation
            .places
            .iter()
            .find(|place| place.place == navigation.cursor.place)
            .ok_or("native front-door current Place is absent")?
            .aspects;
        if aspects.len() < 2 {
            return Ok(());
        }
        let current = aspects
            .iter()
            .position(|aspect| aspect.aspect == navigation.cursor.aspect)
            .ok_or("native front-door current Aspect is absent")?;
        let next = (current as isize + delta).rem_euclid(aspects.len() as isize) as usize;
        self.navigate_front_door(NavigationOperation::Show(aspects[next].aspect))
    }
}
