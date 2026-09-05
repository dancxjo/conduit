//! Toolkit-independent state for the native Patchbay shell.
//!
//! This model projects one ordinary host composition. It never owns a
//! capability registry or accepts UI-authored advertisements.

use conduit_core::{
    BootId, CapabilityId, HostAdvertisement, HostId, Observation, ObservationKind, SignId,
};
use conduit_observatory::{
    CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport,
    ObservatorySnapshot, OfferFreshness, OperationalState, RetentionReport, SNAPSHOT_SCHEMA,
};

mod body_biography;
mod body_biography_entrance;
mod body_planning_session;
mod body_workload_session;
mod build_birth;
mod candidate_form;
mod control;
mod cross_host_renderer;
mod current_body_frame;
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
mod degradation_explanation;
mod degraded_profile_explanation;
mod dormant_readmission_explanation;
mod face_configuration;
mod face_controls;
mod form_editor;
mod form_editor_catalogs;
mod form_editor_error;
mod form_edits;
mod front_door;
mod front_door_projection;
mod front_door_session;
mod front_door_topology;
mod front_door_transition;
mod gear_realization;
mod graphical_patchbay;
mod host_adapter;
mod interaction;
mod layout;
mod learned_watch;
#[cfg(test)]
mod learned_watch_tests;
mod llm_documentary;
mod llm_embodiment_presentation;
mod llm_presentation;
mod llm_replan_explanation;
mod maker_environment;
mod palette;
#[cfg(test)]
mod parts_truth_explanation_tests;
mod parts_view;
mod patchbay_backs;
mod policy_explanation;
mod portable_composition;
mod portable_content;
mod portable_correlations;
mod portable_demo;
mod portable_graph_projection;
mod portable_graphics;
mod portable_layout;
mod portable_navigation;
mod portable_parts_projection;
mod portable_projection;
mod portable_route_projection;
mod portable_vector_search_projection;
mod portable_world_projection;
mod presentation_layout;
mod presenter_plans;
#[cfg(test)]
mod presenter_plans_tests;
mod prewake;
pub mod proof;
mod readable_body_history;
mod recursive_form_projection;
mod recursive_recovery_explanation;
mod renderer_conformance;
mod renderer_execution;
mod renderer_inspection;
mod renderer_projection;
mod route_demo;
mod route_presentation;
mod survival_policy_explanation;
mod text_lab_explanation;
mod text_lab_explanation_loss;
mod topology;
mod topology_hosts;
mod zero_body_authoring;
mod zero_body_front_door;
mod zero_body_projection;

#[cfg(test)]
mod degradation_explanation_tests;

pub use body_biography::{
    project_body_biography, BodyBiographyEntry, BodyBiographyProjection,
    BodyBiographyProjectionError, MAX_BODY_BIOGRAPHY_EXPLANATION_BYTES,
};
pub use body_biography_entrance::{
    PatchbayBodyApplicationEntrance, PatchbayBodyAttachment, PatchbayBodyEntranceError,
    MAX_PATCHBAY_BODY_EVIDENCE_BYTES,
};
pub use body_planning_session::{
    BodyPlanningSession, BodyPlanningSessionError, BodyPlanningSessionSnapshot,
    BodyPlanningTransition,
};
pub use body_workload_session::{
    BodyWorkloadChange, BodyWorkloadChangeKind, PatchbayBodyWorkloadError,
    PatchbayBodyWorkloadSession,
};
pub use build_birth::{
    BirthSigns, BuildBirthController, BuildBirthDocument, BuildBirthError, BuildRevisionStatus,
    PatchbayMode, MAX_BUILD_DOCUMENT_LINES,
};
pub use candidate_form::PatchbayCandidateForm;
pub use conduit_body::WakeLifecycle;
pub use conduit_presentation::{ApplicationTheme, ThemeColor, CONDUIT_APPLICATION_THEME};
pub use control::{
    admit_run, ControlError, ControlReceiptProjection, PatchbayRequestId, PlanDocument,
    PlayDocument, PlayExecutionProjection,
};
pub use cross_host_renderer::{
    cross_host_renderer_plan, CrossHostRendererPlan, CROSS_HOST_MAXIMUM_FRAME_BYTES,
    CROSS_HOST_RENDERER_GEAR, CROSS_HOST_SOURCE_GEAR, PRESENTATION_PROJECT_CAPABILITY,
    PRESENTATION_PROJECT_KIND,
};
pub use current_body_frame::{
    CurrentBodyForm, CurrentBodyFrame, CurrentBodyFrameError, CurrentBodyFrameSlot,
    CurrentBodyHost, CurrentBodyLifecycle, CurrentBodyLifecycleAction, CurrentBodyPatchbayReader,
    CurrentBodyPhysicalHostSummary, CurrentBodyTransition,
};
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
pub use degradation_explanation::{
    PatchbayDegradationExplanation, MAX_DEGRADATION_EXPLANATION_BYTES,
};
pub use degraded_profile_explanation::{
    explain_degraded_profile, explain_degraded_profile_refusal, DegradedProfileExplanation,
    DegradedProfileExplanationError, DegradedProfileState, ProfileDimensionExplanation,
    MAX_DEGRADED_PROFILE_EXPLANATION_BYTES,
};
pub use dormant_readmission_explanation::{
    explain_dormant_readmission, DormantReadmissionExplanation, DormantReadmissionExplanationError,
    MAX_DORMANT_READMISSION_EXPLANATION_BYTES,
};
pub use face_controls::{FaceControl, FaceControlKind, FaceInteraction, MAX_FACE_CONTROLS};
pub use form_editor::{
    CheckedRevision, EditorDiagnostic, FormDocumentView, FormEditor, FormEditorError, GraphCord,
    GraphCordStage, GraphForm, GraphItem, GraphItemKind, SourceSelection,
};
pub use front_door::{
    EntranceAction, EntranceLayer, EntranceRefusal, EntranceUpdateDisposition,
    PatchbayEntranceState, MAX_ENTRANCE_ACTIONS,
};
pub use front_door_session::{LocalFrontDoor, LocalFrontDoorProjection};
pub use front_door_topology::MAX_FRONT_DOOR_LINES;
pub use gear_realization::{
    replan_with_implementation, GearRealizationAlternative, GearRealizationError,
    GearRealizationInspection, RealizationDisposition, MAX_GEAR_REALIZATION_ALTERNATIVES,
};
pub use graphical_patchbay::{
    PatchbayComposition, PatchbayCompositionBinding, PatchbayConnectionCandidate, PatchbayCord,
    PatchbayFacePort, PatchbayGear, PatchbayGraph, PatchbayGraphError, PatchbayInspection,
    PatchbayPort, PatchbayPortCompatibility, PatchbaySubjectKind, PatchbaySubjectRef,
    MAX_PATCHBAY_CORDS, MAX_PATCHBAY_GEARS, MAX_PATCHBAY_PORTS, MAX_PATCHBAY_SUBJECTS,
};
pub use host_adapter::{PatchbayHostAdapter, PatchbayHostExecution, PatchbayHostProfile};
pub use interaction::{
    InteractionDisposition, InteractionError, InteractionReceipt, PatchbayAction, PatchbayEdit,
    PatchbayEditBasis, PatchbayInteraction, PatchbayInteractionRequest,
    PatchbayInteractionRequestId, PatchbayInvocation, PatchbayInvocationOutcome, PatchbayRefusal,
    MAX_INTERACTION_HISTORY, MAX_INTERACTION_ID_BYTES, MAX_INTERACTION_VALUE_BYTES,
};
pub use layout::{
    CordRoute, GearPlacement, PatchbayLayout, PatchbayLayoutError, MAX_GROUP_NAME_BYTES,
    MAX_LAYOUT_COORDINATE, PATCHBAY_LAYOUT_VERSION,
};
pub use learned_watch::{
    ClockAlignment, DynamicsWatch, LearnedWatchProjection, LearnedWatchProjectionKind,
    ObjectiveComponent, ProbabilisticAlternative, ProbabilisticDisposition, ProbabilisticWatch,
    SignalContinuity, SignalPoint, SignalStreamRole, SignalWatch, StateTransition, StateWatch,
    TensorAxis, TensorWatch, TrainingPhase, TrainingWatch, MAX_LEARNED_WATCH_PROJECTIONS,
    MAX_OBJECTIVE_COMPONENTS, MAX_PROBABILISTIC_ALTERNATIVES, MAX_SIGNAL_POINTS, MAX_TENSOR_AXES,
    MAX_TENSOR_SLICE_VALUES,
};
#[cfg(test)]
pub use llm_documentary::llm_documentary_presentation;
pub use llm_documentary::llm_documentary_presentation_with_adapter;
pub use llm_embodiment_presentation::{
    llm_embodiment_documentary_presentations, project_llm_embodiment,
    LlmEmbodimentPresentationError,
};
pub use llm_presentation::{
    project_llm_patchbay, CandidateFormInspection, LlmGearActivity, LlmPatchbayTruth,
    LlmPresentationError, MAXIMUM_LLM_PRESENTATION_STAGES,
};
pub use llm_replan_explanation::{
    explain_cross_host_llm_replan, explain_missing_llm_realization, CrossHostLlmReplanExplanation,
    MAX_LLM_REPLAN_EXPLANATION_BYTES,
};
pub use maker_environment::{
    AuthoredEnvironment, AuthoredEnvironmentError, AuthoredLink, AuthoredPart, ConnectivityKind,
    EnvironmentComparison, EnvironmentComparisonRow, EnvironmentLinkKind, MachineProfile,
    ObservedPartBinding, PartResources, SimulationHostCandidate, SimulationProjection,
    SimulationProvenance, MAKER_ENVIRONMENT_VERSION, MAX_AUTHORED_LINKS, MAX_AUTHORED_PARTS,
    MAX_ENVIRONMENT_COORDINATE, MAX_ENVIRONMENT_ID_BYTES, MAX_PART_NAME_BYTES,
};
pub use palette::{
    GearPalette, PaletteCategory, PaletteConfigurationSummary, PaletteEntry, PaletteError,
    PaletteIconKey, MAX_PALETTE_ENTRIES, MAX_PALETTE_QUERY_BYTES,
};
pub use parts_view::*;
pub use patchbay_backs::*;
pub use policy_explanation::{
    PolicyChoiceDetails, PolicyChoiceDomain, PolicyChoiceExplanation, PolicyChoiceSummary,
    PolicyExplanationError, PolicyReplanRequest, MAX_POLICY_EXPLANATIONS,
    MAX_STYLE_EXPLANATION_CLAUSES,
};
pub use portable_composition::{
    constrained_frame_layout, constrained_graphics_scene, DirectObligation, DirectPresentation,
};
#[cfg(test)]
pub use portable_demo::{portable_demonstration, portable_demonstration_with_parts};
pub use portable_demo::{
    portable_demonstration_with_adapter, portable_demonstration_with_parts_and_adapter,
};
pub use portable_graphics::{NativeGraphicsObligation, NativeGraphicsPresenter};
pub use portable_layout::{DirectLayoutEvaluator, DirectLayoutOperation};
pub use portable_navigation::PatchbayNavigationProjection;
pub use portable_projection::PortableProjectionError;
pub use presentation_layout::{
    fit_measured_text, LayoutCollision, MeasuredTextFit, PresentationLayoutError,
    PresentationOverflow, PresentationPriority, PresentationRegion, PresentationRegionId,
    PresentationRegionMode, ResponsivePatchbayLayout, MAX_PRESENTATION_REGIONS,
};
pub use presenter_plans::*;
pub use prewake::*;
pub use readable_body_history::{
    BodyHistoryAccess, BodyHistoryEntry, BodyHistoryExactEvidence, BodyHistoryInspectTarget,
    BodyHistoryManifestation, BodyHistoryMoment, ReadableBodyHistory, ReadableBodyHistoryError,
    ReadableBodyHistorySlot, MAX_BODY_HISTORY_LINEAR_BYTES, MAX_BODY_HISTORY_TITLE_BYTES,
};
pub use recursive_form_projection::{
    project_recursive_form_gear, RecursiveFormGearProjection, RecursiveFormProjectionError,
};
pub use recursive_recovery_explanation::{
    explain_recursive_recovery, RecursiveRecoveryExplanation, RecursiveRecoveryExplanationError,
    MAX_RECURSIVE_RECOVERY_EXPLANATION_BYTES,
};
pub use renderer_conformance::{
    compare_entrances, EntranceEquivalenceError, EntranceEquivalenceReport,
    ENTRANCE_EQUIVALENCE_SCHEMA,
};
pub use renderer_execution::{
    RendererAdapterIdentity, RendererAdapterKind, RendererExecution, RendererExecutionError,
};
pub use renderer_inspection::{RendererSelfInspection, RendererSelfInspectionError};
pub use renderer_projection::{
    AttemptedEditPresentation, PatchbayPresentation, RendererIdentityProjection,
    RendererProjectionError, MAX_RENDERER_DIAGNOSTICS, MAX_RENDERER_GRAPH_ITEMS,
    MAX_RENDERER_INSPECTION_LINES, MAX_RENDERER_PLAN_ITEMS, MAX_RENDERER_ROUTES,
    MAX_RENDERER_ROUTE_CANDIDATES, MAX_RENDERER_SIGNS, MAX_RENDERER_TOPOLOGY_ITEMS,
};
pub use route_demo::{DistributedRouteDemo, RouteDemoError};
pub use route_presentation::{
    DistributedRoutePresentation, NewPlanRecoveryPresentation, RefusedRoutePresentation,
    RouteCandidatePresentation, RoutePlanPresentation, SamePlanFallbackPresentation,
};
pub use survival_policy_explanation::{
    explain_survival_plan_selection, SurvivalPolicyExplanation, SurvivalPolicyExplanationError,
    MAX_SURVIVAL_POLICY_EXPLANATION_BYTES,
};
pub use text_lab_explanation::{
    text_lab_split_explanation, text_lab_split_loss_explanation, TextLabSplitExplanation,
};
pub use topology::{PatchbayTopology, TopologyDocument, TopologyViewError};
pub use topology_hosts::current_device_for_capability;
pub use zero_body_front_door::{
    BodyJoinCandidate, FormCandidate, OpenedFrontDoorSubject, ZeroBodyFrontDoor,
    ZeroBodyFrontDoorProjection, MAX_FRONT_DOOR_BODY_CANDIDATES, MAX_FRONT_DOOR_FORMS,
    MAX_FRONT_DOOR_REFUSAL_SIGNS,
};
pub const MAX_FORM_SOURCE_BYTES: usize = conduit_form::MAXIMUM_FORM_SOURCE_BYTES;

#[cfg(test)]
mod build_birth_tests;
#[cfg(test)]
mod face_configuration_tests;
#[cfg(test)]
mod front_door_session_tests;
#[cfg(test)]
mod front_door_tests;
#[cfg(test)]
mod gear_realization_tests;
#[cfg(test)]
mod graphical_patchbay_tests;
#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod maker_environment_tests;
#[cfg(test)]
mod parts_view_tests;
#[cfg(test)]
mod policy_explanation_tests;
#[cfg(test)]
mod portable_navigation_tests;
#[cfg(test)]
mod portable_parts_temporal_tests;
#[cfg(test)]
mod portable_projection_tests;
#[cfg(test)]
mod portable_vector_search_projection_tests;
#[cfg(test)]
mod prewake_tests;
#[cfg(test)]
mod renderer_execution_tests;
#[cfg(test)]
mod text_lab_explanation_tests;
#[cfg(test)]
mod theme_tests;

const LIFECYCLE_CAPACITY: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProjection {
    host_id: HostId,
    boot_id: BootId,
    capability_ids: Vec<CapabilityId>,
    planner_profile_count: usize,
}

impl HostProjection {
    fn from_advertisement(advertisement: &HostAdvertisement) -> Self {
        Self {
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            capability_ids: advertisement
                .capabilities
                .iter()
                .map(|offer| offer.capability_id.clone())
                .collect(),
            planner_profile_count: advertisement.planner_capabilities.len(),
        }
    }

    pub fn host_id(&self) -> &HostId {
        &self.host_id
    }

    pub fn boot_id(&self) -> &BootId {
        &self.boot_id
    }

    pub fn capability_ids(&self) -> &[CapabilityId] {
        &self.capability_ids
    }

    pub fn planner_profile_count(&self) -> usize {
        self.planner_profile_count
    }
}

#[derive(Clone)]
pub struct PatchbayModel {
    advertisement: HostAdvertisement,
    projection: HostProjection,
}

impl PatchbayModel {
    /// Projects one authoritative Host advertisement supplied by the
    /// application composition edge. Patchbay does not construct Hosts.
    pub fn from_advertisement(advertisement: HostAdvertisement) -> Self {
        let projection = HostProjection::from_advertisement(&advertisement);
        Self {
            advertisement,
            projection,
        }
    }

    #[cfg(test)]
    pub fn with_identity(host_id: HostId, boot_id: BootId) -> Self {
        let advertisement = crate::host_adapter::test_host_adapter()
            .advertisement(
                host_id,
                boot_id,
                conduit_core::OfferGeneration(1),
                crate::PatchbayHostProfile::Signal,
            )
            .expect("test Host advertisement");
        Self::from_advertisement(advertisement)
    }

    pub fn projection(&self) -> &HostProjection {
        &self.projection
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }

    pub fn startup_snapshot(&self) -> ObservatorySnapshot {
        self.snapshot(
            OperationalState::Available,
            vec![
                self.observation(0, ObservationKind::HostStarted),
                self.observation(1, ObservationKind::AdvertisementPublished),
            ],
        )
    }

    /// Final bounded report emitted before the native event loop exits.
    pub fn shutdown_snapshot(&self) -> ObservatorySnapshot {
        self.snapshot(OperationalState::Unreachable, Vec::new())
    }

    fn observation(&self, sequence: u64, kind: ObservationKind) -> Observation {
        Observation {
            sign_id: SignId::from(format!(
                "patchbay-lifecycle/{}/{}",
                self.projection.boot_id().as_str(),
                sequence
            )),
            active_play_id: None,
            presentation_id: None,
            host_id: self.projection.host_id().clone(),
            boot_id: self.projection.boot_id().clone(),
            plan_id: None,
            placement_id: None,
            connection_id: None,
            kind,
        }
    }

    fn snapshot(
        &self,
        state: OperationalState,
        observations: Vec<Observation>,
    ) -> ObservatorySnapshot {
        let advertisement = self.advertisement().clone();
        let capabilities = advertisement
            .capabilities
            .iter()
            .map(|offer| CapabilityStatusReport {
                capability_id: offer.capability_id.clone(),
                freshness: OfferFreshness::Fresh,
                support: CapabilitySupport::Supported,
                availability: if state == OperationalState::Available {
                    CapabilityAvailability::Available
                } else {
                    CapabilityAvailability::Unavailable
                },
            })
            .collect();
        ObservatorySnapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            hosts: vec![HostReport {
                advertisement,
                state,
                capabilities,
                devices: Vec::new(),
            }],
            bases: Vec::new(),
            lines: Vec::new(),
            plans: Vec::new(),
            plays: Vec::new(),
            retention: RetentionReport {
                item_capacity: LIFECYCLE_CAPACITY,
                retained_items: observations.len() as u32,
                dropped_items: 0,
            },
            observations,
            historical_observations: Vec::new(),
            sealed_boot_provenance: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_observatory::validate_snapshot;
    use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};

    fn model() -> PatchbayModel {
        let host = StdHost::new_with_composition(
            StdHostConfig {
                host_id: HostId::from("patchbay-test-host"),
                boot_id: BootId::from("patchbay-test-boot"),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            StdHostComposition::minimal().with_signal(),
        );
        PatchbayModel::from_advertisement(host.advertisement().clone())
    }

    #[test]
    fn projection_can_only_report_the_composed_advertisement() {
        let model = model();
        let advertised = model
            .advertisement()
            .capabilities
            .iter()
            .map(|offer| offer.capability_id.clone())
            .collect::<Vec<_>>();

        assert_eq!(model.projection().capability_ids(), advertised);
        assert!(advertised.iter().any(|id| id.as_str() == "pulse-1"));
        assert_eq!(model.projection().planner_profile_count(), 1);
    }

    #[test]
    fn lifecycle_reports_are_bounded_current_model_snapshots() {
        let model = model();
        let startup = model.startup_snapshot();
        let shutdown = model.shutdown_snapshot();

        validate_snapshot(&startup).expect("startup snapshot is valid");
        validate_snapshot(&shutdown).expect("shutdown snapshot is valid");
        assert_eq!(startup.retention.item_capacity, LIFECYCLE_CAPACITY);
        assert_eq!(startup.retention.retained_items, 2);
        assert!(matches!(
            startup.observations[0].kind,
            ObservationKind::HostStarted
        ));
        assert!(matches!(
            startup.observations[1].kind,
            ObservationKind::AdvertisementPublished
        ));
        assert_eq!(shutdown.hosts[0].state, OperationalState::Unreachable);
        assert_eq!(shutdown.retention.retained_items, 0);
    }

    #[test]
    fn model_preserves_the_exact_application_supplied_identity() {
        let model = model();
        assert_eq!(model.projection().host_id().as_str(), "patchbay-test-host");
        assert_eq!(model.projection().boot_id().as_str(), "patchbay-test-boot");
    }
}
