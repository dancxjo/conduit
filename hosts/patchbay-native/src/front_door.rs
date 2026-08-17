//! Native consumption of the shared truthful Patchbay entrance state.

use super::PatchbayApplication;
use conduit_core::SignId;
use conduit_presentation::{NavigationOperation, NavigationState, Presentation};
use patchbay_model::{
    PatchbayEntranceState, PatchbayNavigationProjection, RendererAdapterIdentity,
    RendererAdapterKind, RendererExecution, ZeroBodyFrontDoor,
};

pub(super) struct NativeFrontDoorPresentation {
    pub state: PatchbayEntranceState,
    pub presentation: Presentation,
    pub navigation: PatchbayNavigationProjection,
    navigation_state: NavigationState,
}

impl NativeFrontDoorPresentation {
    fn new(
        state: PatchbayEntranceState,
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
            state,
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
        let state = PatchbayEntranceState::enter(&presentation)
            .map_err(|error| format!("front-door state: {error:?}"))?;
        let execution = self.prepare_front_door_renderer(presentation.clone())?;
        self.zero_body_front_door = Some(session);
        self.entrance = Some(NativeFrontDoorPresentation::new(
            state,
            presentation,
            navigation,
        )?);
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
        let mut state = self
            .entrance
            .as_ref()
            .map(|entrance| entrance.state.clone())
            .ok_or("native front-door state is absent")?;
        state
            .update(&presentation)
            .map_err(|error| format!("front-door update: {error:?}"))?;
        let execution = self.prepare_front_door_renderer(presentation.clone())?;
        self.entrance = Some(NativeFrontDoorPresentation::new(
            state,
            presentation,
            navigation,
        )?);
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
                entrance
                    .state
                    .select(&entrance.presentation, subject)
                    .map_err(|error| format!("front-door selection: {error:?}"))?;
                entrance.navigation.cursor = cursor;
                entrance.navigation_state = next_navigation;
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
