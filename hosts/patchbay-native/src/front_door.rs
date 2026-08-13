//! Native consumption of the shared truthful Patchbay entrance state.

use super::PatchbayApplication;
use conduit_core::SignId;
use conduit_presentation::Presentation;
use patchbay_model::{
    PatchbayEntranceState, RendererAdapterIdentity, RendererAdapterKind, RendererExecution,
    ZeroBodyFrontDoor,
};

impl PatchbayApplication {
    pub(super) fn initialize_front_door(&mut self) -> Result<(), String> {
        let session = ZeroBodyFrontDoor::from_model(self.model.clone())?;
        let presentation = session.project()?.presentation;
        let state = PatchbayEntranceState::enter(&presentation)
            .map_err(|error| format!("front-door state: {error:?}"))?;
        let execution = self.prepare_front_door_renderer(presentation.clone())?;
        self.zero_body_front_door = Some(session);
        self.entrance_presentation = Some(presentation);
        self.entrance_state = Some(state);
        self.renderer_execution = Some(execution);
        Ok(())
    }

    pub(super) fn refresh_front_door(&mut self) -> Result<(), String> {
        let Some(session) = self.zero_body_front_door.as_ref() else {
            return Ok(());
        };
        let presentation = session.project()?.presentation;
        if self.entrance_presentation.as_ref().is_some_and(|prior| {
            prior.revision == presentation.revision && prior.identity == presentation.identity
        }) {
            return Ok(());
        }
        let mut state = self
            .entrance_state
            .clone()
            .ok_or("native front-door state is absent")?;
        state
            .update(&presentation)
            .map_err(|error| format!("front-door update: {error:?}"))?;
        let execution = self.prepare_front_door_renderer(presentation.clone())?;
        self.entrance_presentation = Some(presentation);
        self.entrance_state = Some(state);
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
        match (&mut self.entrance_state, &self.entrance_presentation) {
            (Some(state), Some(presentation)) => state
                .select(presentation, subject)
                .map_err(|error| format!("front-door selection: {error:?}")),
            (None, None) => Ok(()),
            _ => Err("front-door state and presentation became inconsistent".into()),
        }
    }
}
