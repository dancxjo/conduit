//! Retained, renderer-neutral Patchbay application over exact plot/Plan truth.
#![no_std]

extern crate alloc;

use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{ActivePlayId, PlanId};
use conduit_plot::ExpandedCanonicalPlot;
use conduit_presentation::{
    ApplicationAction, ApplicationComponent, ApplicationEvent, ApplicationEventKind,
    ApplicationNodeState, ApplicationView, ApplicationViewNode, ApplicationViewRefusal,
};
use patchbay_control::PatchbayAction;
use patchbay_graph::{PatchbayGraph, PatchbayGraphError, PatchbayInspection};

pub const INSPECT_NEXT_ACTION_ID: &str = "patchbay.inspect.next";
pub const EDIT_CURRENT_ACTION_ID: &str = "patchbay.edit.current";
pub const SELECT_PLOT_ACTION_PREFIX: &str = "patchbay.plot.";
mod debugger_control;
#[cfg(test)]
mod debugger_control_tests;
mod debugger_presentation;
#[cfg(test)]
mod debugger_presentation_tests;
mod debugger_timeline;
#[cfg(test)]
mod debugger_timeline_tests;
mod debugger_watch;
#[cfg(test)]
mod debugger_watch_tests;
mod entrance;
mod entrance_equivalence;
mod learned_watch;
#[cfg(test)]
mod learned_watch_tests;
mod mask;
mod navigation_projection;
mod parts_projection;
mod portable_composition;
mod portable_graphics;
mod portable_layout;
mod presentation_layout;
mod resident_canvas;
mod resident_view;
pub use mask::{PatchbayMaskMode, PatchbayMaskStage, PatchbayMaskTopology, CHANGE_MASKS_ACTION_ID};
mod inspection;
pub use inspection::{
    project_mask_inspection, MaskInspectionError, MaskInspectionProjection, MaskInspectionRoute,
    MaskInspectionShow,
};
mod topology;
mod topology_projection;
mod workbench_layout;
pub use debugger_control::{
    DebuggerExecutionControl, DebuggerExecutionControlState, DEBUGGER_CONTROL_SCHEMA,
    MAX_DEBUGGER_BREAKPOINT_SUBJECTS, MAX_DEBUGGER_CONTROL_REASON_BYTES,
};
pub use debugger_presentation::{
    DebuggerActivityPhase, DebuggerExecutionIdentity, DebuggerGapPresentation,
    DebuggerPresentation, DebuggerPresentationError, DebuggerSubjectActivity,
    DebuggerSubjectBinding, DebuggerValueKind, DebuggerValuePresentation,
    DEBUGGER_PRESENTATION_SCHEMA, MAX_DEBUGGER_SUBJECTS, MAX_DEBUGGER_SUMMARY_BYTES,
    RECENT_ACTIVITY_TICKS,
};
pub use debugger_timeline::{
    DebuggerCausalTrace, DebuggerTimeline, DebuggerTimelineBinding, DebuggerTimelineError,
    DebuggerTimelineEvent, DebuggerTimelineMode, DebuggerTimelineProjection,
    DebuggerTimelineSubjectState, DebuggerTimelineWatchState, DebuggerTraceDirection,
    DebuggerTraceStep, DEBUGGER_TIMELINE_SCHEMA, MAX_DEBUGGER_TIMELINE_BYTES,
    MAX_DEBUGGER_TIMELINE_EVENTS,
};
pub use debugger_watch::{
    DebuggerWatch, DebuggerWatchBinding, DebuggerWatchError, DebuggerWatchHistoryEntry,
    DebuggerWatchLifecycle, DebuggerWatchRate, DebuggerWatchSet, DebuggerWatchSubjectRole,
    DEBUGGER_WATCH_SCHEMA, MAX_DEBUGGER_WATCHES, MAX_WATCH_HISTORY_RECORDS,
};
pub use entrance::{
    EntranceAction, EntranceLayer, EntranceRefusal, EntranceUpdateDisposition,
    PatchbayEntranceState, MAX_ENTRANCE_ACTIONS,
};
pub use entrance_equivalence::{
    compare_entrances, EntranceEquivalenceError, EntranceEquivalenceReport,
    ENTRANCE_EQUIVALENCE_SCHEMA, ONE_PLOT_TWO_FACES_BOOT_ID, ONE_PLOT_TWO_FACES_HOST_ID,
};
pub use learned_watch::{
    ClockAlignment, DynamicsWatch, LearnedLifecyclePhase, LearnedLifecycleWatch,
    LearnedWatchProjection, LearnedWatchProjectionKind, ObjectiveComponent,
    ProbabilisticAlternative, ProbabilisticDisposition, ProbabilisticWatch, SignalContinuity,
    SignalPoint, SignalStreamRole, SignalWatch, StateTransition, StateWatch, TensorAxis,
    TensorWatch, TrainingPhase, TrainingWatch, MAX_LEARNED_WATCH_PROJECTIONS,
    MAX_OBJECTIVE_COMPONENTS, MAX_PROBABILISTIC_ALTERNATIVES, MAX_SIGNAL_POINTS, MAX_TENSOR_AXES,
    MAX_TENSOR_SLICE_VALUES,
};
pub use navigation_projection::PatchbayNavigationProjection;
pub use parts_projection::*;
pub use portable_composition::{
    constrained_frame_layout, constrained_graphics_scene, DirectObligation, DirectPresentation,
};
pub use portable_graphics::{NativeGraphicsMask, NativeGraphicsObligation};
pub use portable_layout::{DirectLayoutEvaluator, DirectLayoutOperation};
pub use presentation_layout::{
    fit_measured_text, LayoutCollision, MeasuredTextFit, PresentationLayoutError,
    PresentationOverflow, PresentationPriority, PresentationRegion, PresentationRegionId,
    PresentationRegionMode, ResponsivePatchbayLayout, MAX_PRESENTATION_REGIONS,
};
pub use topology::*;
pub use topology_projection::project_mask_topology;
pub use workbench_layout::{
    CordRoute, GearPlacement, PatchbayLayout, PatchbayLayoutError, MAX_GROUP_NAME_BYTES,
    MAX_LAYOUT_COORDINATE, PATCHBAY_LAYOUT_VERSION,
};
#[cfg(test)]
mod topology_tests;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatchbayApplicationRequest {
    EditCurrent {
        expanded_plot_id: conduit_core::ExpandedPlotId,
        subject_identity: String,
    },
    ChangeMasks {
        body_plan_id: PlanId,
        mode: PatchbayMaskMode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatchbayApplicationOutput {
    pub view: Vec<u8>,
    /// Authority-bearing changes are requests against authoritative Body truth.
    pub request: Option<PatchbayApplicationRequest>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatchbayApplicationRefusal {
    Graph(PatchbayGraphError),
    Event(ApplicationViewRefusal),
    UnknownAction,
    NoSubject,
    RevisionExhausted,
    EmptyActivePlots,
    TooManyActivePlots,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchbayActivePlotState {
    Playing,
    Lulled,
}

pub struct PatchbayActivePlot {
    graph: PatchbayGraph,
    title: String,
    checked_plot_id: String,
    plan_id: PlanId,
    state: PatchbayActivePlotState,
    focused: bool,
}

impl PatchbayActivePlot {
    pub fn project(
        plot: &ExpandedCanonicalPlot,
        title: impl Into<String>,
        checked_plot_id: impl Into<String>,
        plan_id: PlanId,
        state: PatchbayActivePlotState,
        focused: bool,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        Ok(Self {
            graph: PatchbayGraph::from_expanded(plot).map_err(PatchbayApplicationRefusal::Graph)?,
            title: title.into(),
            checked_plot_id: checked_plot_id.into(),
            plan_id,
            state,
            focused,
        })
    }
}

/// Representation delivery, not a change to the resident graph or authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PatchbayCanvasDelivery {
    /// Include a bounded graph for a renderer that consumes application canvases.
    InlineGraph,
    /// Retain exact textual inspection for renderers without inline graph support.
    TextInspection,
}

pub struct PatchbayApplicationPort {
    plots: Vec<PatchbayActivePlot>,
    canvas_delivery: PatchbayCanvasDelivery,
    body_plan_id: PlanId,
    active_play_id: Option<ActivePlayId>,
    revision: u32,
    selected_plot: Option<usize>,
    selected_subject: usize,
    edit_requested: bool,
    mask_topology: Option<PatchbayMaskTopology>,
}

impl PatchbayApplicationPort {
    pub fn open(
        plot: &ExpandedCanonicalPlot,
        plan_id: PlanId,
        body_plan_id: PlanId,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        let checked_plot_id = String::from(plot.checked_plot_id.as_str());
        let entry = PatchbayActivePlot::project(
            plot,
            plot.name.clone(),
            checked_plot_id,
            plan_id,
            PatchbayActivePlotState::Playing,
            true,
        )?;
        let mut port = Self::open_active(vec![entry], body_plan_id, None)?;
        port.selected_plot = Some(0);
        Ok(port)
    }

    pub fn open_active(
        plots: Vec<PatchbayActivePlot>,
        body_plan_id: PlanId,
        active_play_id: Option<ActivePlayId>,
    ) -> Result<Self, PatchbayApplicationRefusal> {
        if plots.is_empty() {
            return Err(PatchbayApplicationRefusal::EmptyActivePlots);
        }
        if plots.len() + 4 > conduit_presentation::MAX_APPLICATION_ACTIONS {
            return Err(PatchbayApplicationRefusal::TooManyActivePlots);
        }
        Ok(Self {
            plots,
            canvas_delivery: PatchbayCanvasDelivery::InlineGraph,
            body_plan_id,
            active_play_id,
            revision: 1,
            selected_plot: None,
            selected_subject: 0,
            edit_requested: false,
            mask_topology: None,
        })
    }

    /// Choose the representation supported by the admitted renderer at preparation.
    pub fn with_canvas_delivery(mut self, delivery: PatchbayCanvasDelivery) -> Self {
        self.canvas_delivery = delivery;
        self
    }

    pub fn set_mask_topology(&mut self, topology: PatchbayMaskTopology) {
        self.body_plan_id = topology.body_plan_id.clone();
        self.mask_topology = Some(topology);
    }

    pub fn graph(&self) -> &PatchbayGraph {
        &self.plots[0].graph
    }
    pub fn plan_id(&self) -> &PlanId {
        &self.plots[0].plan_id
    }

    pub fn inspection(&self) -> Result<PatchbayInspection, PatchbayApplicationRefusal> {
        let plot = self.selected_active_plot()?;
        let subject = plot
            .graph
            .subject_identities()
            .nth(self.selected_subject)
            .ok_or(PatchbayApplicationRefusal::NoSubject)?;
        plot.graph
            .inspect(subject)
            .map_err(PatchbayApplicationRefusal::Graph)
    }

    pub fn apply(
        &mut self,
        encoded: &[u8],
    ) -> Result<PatchbayApplicationOutput, PatchbayApplicationRefusal> {
        let current = self.view()?;
        let request = if encoded.is_empty() {
            None
        } else {
            let event = ApplicationEvent::decode(encoded, &current)
                .map_err(PatchbayApplicationRefusal::Event)?;
            self.apply_event(&event)?
        };
        Ok(PatchbayApplicationOutput {
            view: self
                .view()?
                .encode()
                .map_err(PatchbayApplicationRefusal::Event)?,
            request,
        })
    }

    fn apply_event(
        &mut self,
        event: &ApplicationEvent,
    ) -> Result<Option<PatchbayApplicationRequest>, PatchbayApplicationRefusal> {
        if event.action == resident_canvas::SELECT_SUBJECT
            && event.kind == ApplicationEventKind::Change
        {
            self.select_canvas_subject(&event.value)?;
            return Ok(None);
        }
        if event.kind != ApplicationEventKind::Activate {
            return Err(PatchbayApplicationRefusal::UnknownAction);
        }
        match event.action.as_str() {
            INSPECT_NEXT_ACTION_ID => {
                let count = self
                    .selected_active_plot()?
                    .graph
                    .subject_identities()
                    .count();
                if count == 0 {
                    return Err(PatchbayApplicationRefusal::NoSubject);
                }
                self.selected_subject = (self.selected_subject + 1) % count;
                self.revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(PatchbayApplicationRefusal::RevisionExhausted)?;
                Ok(None)
            }
            EDIT_CURRENT_ACTION_ID => {
                let inspection = self.inspection()?;
                // Reuse the shared semantic action identity; realization remains
                // outside application state and cannot acquire ambient authority.
                debug_assert_eq!(PatchbayAction::ConfigureGear.as_str(), "configure-gear");
                self.edit_requested = true;
                self.revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(PatchbayApplicationRefusal::RevisionExhausted)?;
                Ok(Some(PatchbayApplicationRequest::EditCurrent {
                    expanded_plot_id: self.selected_active_plot()?.graph.expanded_plot_id.clone(),
                    subject_identity: inspection.subject_identity,
                }))
            }
            CHANGE_MASKS_ACTION_ID => {
                let topology = self
                    .mask_topology
                    .as_ref()
                    .ok_or(PatchbayApplicationRefusal::UnknownAction)?;
                let mode = topology.next_mode();
                Ok(Some(PatchbayApplicationRequest::ChangeMasks {
                    body_plan_id: topology.body_plan_id.clone(),
                    mode,
                }))
            }
            action if action.starts_with(SELECT_PLOT_ACTION_PREFIX) => {
                let index = action[SELECT_PLOT_ACTION_PREFIX.len()..]
                    .parse::<usize>()
                    .map_err(|_| PatchbayApplicationRefusal::UnknownAction)?;
                if index >= self.plots.len() {
                    return Err(PatchbayApplicationRefusal::UnknownAction);
                }
                self.selected_plot = Some(index);
                self.selected_subject = 0;
                self.edit_requested = false;
                self.revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(PatchbayApplicationRefusal::RevisionExhausted)?;
                Ok(None)
            }
            _ => Err(PatchbayApplicationRefusal::UnknownAction),
        }
    }

    fn selected_active_plot(&self) -> Result<&PatchbayActivePlot, PatchbayApplicationRefusal> {
        self.selected_plot
            .and_then(|index| self.plots.get(index))
            .ok_or(PatchbayApplicationRefusal::NoSubject)
    }
}

#[cfg(test)]
mod tests;
