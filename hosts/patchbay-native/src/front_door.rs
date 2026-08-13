//! Native consumption of the shared world-first Patchbay entrance state.

use super::PatchbayApplication;
use conduit_core::SignId;
use conduit_presentation::Presentation;
use patchbay_model::{
    PatchbayEntranceState, PatchbayGraph, PatchbayPresentation, RendererAdapterIdentity,
    RendererAdapterKind, RendererExecution,
};

impl PatchbayApplication {
    pub(super) fn initialize_front_door(&mut self) -> Result<(), String> {
        let presentation = self.project_front_door(1)?;
        let state = PatchbayEntranceState::enter(&presentation)
            .map_err(|error| format!("front-door state: {error:?}"))?;
        let execution = self.prepare_front_door_renderer(presentation.clone())?;
        self.entrance_presentation = Some(presentation);
        self.entrance_state = Some(state);
        self.renderer_execution = Some(execution);
        Ok(())
    }

    pub(super) fn refresh_front_door(&mut self) -> Result<(), String> {
        let Some(previous) = self.entrance_presentation.as_ref() else {
            return Ok(());
        };
        let revision = previous
            .revision
            .checked_add(1)
            .ok_or("native front-door presentation revision exhausted")?;
        let presentation = self.project_front_door(revision)?;
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

    fn project_front_door(&self, revision: u64) -> Result<Presentation, String> {
        let editor = self
            .form_editor
            .as_ref()
            .ok_or("Patchbay front door requires its canonical entrance Form")?;
        let body = self
            .build_birth
            .body()
            .ok_or("Patchbay front door requires a born Body")?;
        let wake = self
            .build_birth
            .wake_value()
            .ok_or("Patchbay front door requires an awake Body")?;
        let parts = self
            .parts_projection()?
            .ok_or("Patchbay front door requires canonical Parts truth")?;
        let topology = conduit_observatory::build_report(&self.model.startup_snapshot())?;
        let projection = PatchbayPresentation::new(
            revision,
            editor.view(),
            self.control.plan_document().cloned(),
            self.control.play_document().cloned(),
            Some(topology),
            Vec::new(),
        )
        .map_err(|error| error.to_string())?
        .with_graph(
            PatchbayGraph::from_expanded(
                &editor
                    .expand_form(&editor.view().open_form)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        projection
            .to_portable_front_door(body, wake, &parts)
            .map_err(|error| error.to_string())
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
