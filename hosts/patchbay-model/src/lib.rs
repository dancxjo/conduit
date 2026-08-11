//! Toolkit-independent state for the native Patchbay shell.
//!
//! This model projects one ordinary host composition. It never owns a
//! capability registry or accepts UI-authored advertisements.

use conduit_core::{
    BootId, CapabilityId, HostAdvertisement, HostId, Observation, ObservationKind, OfferGeneration,
    SignId,
};
use conduit_observatory::{
    CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport,
    ObservatorySnapshot, OfferFreshness, OperationalState, RetentionReport, SNAPSHOT_SCHEMA,
};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

mod build_birth;
mod control;
mod cross_host_renderer;
mod face_configuration;
mod face_controls;
mod form_editor;
mod form_editor_error;
mod form_edits;
mod graphical_patchbay;
mod interaction;
mod layout;
mod maker_environment;
mod palette;
mod portable_demo;
mod portable_projection;
mod renderer_execution;
mod renderer_inspection;
mod renderer_projection;
mod route_demo;
mod route_presentation;
mod theme;
mod topology;

pub use build_birth::{
    BuildBirthController, BuildBirthDocument, BuildBirthError, BuildRevisionStatus, PatchbayMode,
    MAX_BUILD_DOCUMENT_LINES,
};
pub use control::{admit_run, ControlError, PatchbayRequestId, PlanDocument, PlayDocument};
pub use cross_host_renderer::{
    cross_host_renderer_plan, CrossHostRendererPlan, CROSS_HOST_MAXIMUM_FRAME_BYTES,
    CROSS_HOST_RENDERER_GEAR, CROSS_HOST_SOURCE_GEAR, PRESENTATION_PROJECT_CAPABILITY,
    PRESENTATION_PROJECT_KIND,
};
pub use face_controls::{FaceControl, FaceControlKind, MAX_FACE_CONTROLS};
pub use form_editor::{
    CheckedRevision, EditorDiagnostic, FormDocumentView, FormEditor, FormEditorError, GraphCord,
    GraphCordStage, GraphForm, GraphItem, GraphItemKind, SourceSelection,
};
pub use graphical_patchbay::{
    PatchbayCord, PatchbayGear, PatchbayGraph, PatchbayGraphError, PatchbayInspection,
    PatchbayPort, PatchbaySubjectKind, PatchbaySubjectRef, MAX_PATCHBAY_CORDS, MAX_PATCHBAY_GEARS,
    MAX_PATCHBAY_PORTS,
};
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
pub use portable_demo::portable_demonstration;
pub use portable_projection::PortableProjectionError;
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
pub use theme::{PatchbayTheme, ThemeColor, PHOSPHOR_THEME};
pub use topology::{PatchbayTopology, TopologyDocument, TopologyViewError};
pub const MAX_FORM_SOURCE_BYTES: usize = conduit_form::MAXIMUM_FORM_SOURCE_BYTES;

#[cfg(test)]
mod build_birth_tests;
#[cfg(test)]
mod face_configuration_tests;
#[cfg(test)]
mod graphical_patchbay_tests;
#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod maker_environment_tests;
#[cfg(test)]
mod portable_projection_tests;
#[cfg(test)]
mod renderer_execution_tests;
#[cfg(test)]
mod theme_tests;

const LIFECYCLE_CAPACITY: u32 = 2;
static ID_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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

pub struct PatchbayModel {
    host: StdHost,
    projection: HostProjection,
}

impl PatchbayModel {
    /// Creates a fresh process-scoped host and boot identity.
    pub fn fresh() -> Self {
        Self::fresh_with_composition(StdHostComposition::minimal().with_signal())
    }

    /// Creates a fresh process-scoped identity with the exact native host image.
    pub fn fresh_with_composition(composition: StdHostComposition) -> Self {
        let nonce = fresh_nonce();
        Self::with_identity_and_composition(
            HostId::from(format!("patchbay-native/{nonce}")),
            BootId::from(format!("patchbay-boot/{nonce}")),
            composition,
        )
    }

    /// Deterministic constructor for conformance tests and embedding.
    pub fn with_identity(host_id: HostId, boot_id: BootId) -> Self {
        Self::with_identity_and_composition(
            host_id,
            boot_id,
            StdHostComposition::minimal().with_signal(),
        )
    }

    pub fn with_identity_and_composition(
        host_id: HostId,
        boot_id: BootId,
        composition: StdHostComposition,
    ) -> Self {
        let host = StdHost::new_with_composition(
            StdHostConfig {
                host_id,
                boot_id,
                offer_generation: OfferGeneration(1),
            },
            composition,
        );
        let projection = HostProjection::from_advertisement(host.advertisement());
        Self { host, projection }
    }

    pub fn projection(&self) -> &HostProjection {
        &self.projection
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        self.host.advertisement()
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

fn fresh_nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = ID_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{nanos:x}-{sequence:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_observatory::validate_snapshot;

    fn model() -> PatchbayModel {
        PatchbayModel::with_identity(
            HostId::from("patchbay-test-host"),
            BootId::from("patchbay-test-boot"),
        )
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
    fn fresh_processes_get_distinct_exact_identities() {
        let first = PatchbayModel::fresh();
        let second = PatchbayModel::fresh();

        assert_ne!(first.projection().host_id(), second.projection().host_id());
        assert_ne!(first.projection().boot_id(), second.projection().boot_id());
    }
}
