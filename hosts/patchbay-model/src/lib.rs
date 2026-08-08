//! Toolkit-independent state for the native Patchbay shell.
//!
//! This model projects one ordinary host composition. It never owns a
//! capability registry or accepts UI-authored advertisements.

use conduit_core::{
    BootId, CapabilityId, EvidenceId, HostAdvertisement, HostId, Observation, ObservationKind,
    OfferGeneration,
};
use conduit_observatory::{
    CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport,
    ObservatorySnapshot, OfferFreshness, OperationalState, RetentionReport, SNAPSHOT_SCHEMA,
};
use conduit_std_host::{StdHost, StdHostComposition, StdHostConfig};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

mod control;
mod form_editor;
mod topology;

pub use control::{admit_run, ControlError, PatchbayRequestId, PlanDocument, PlayDocument};
pub use form_editor::{
    CheckedRevision, EditorDiagnostic, FormDocumentView, FormEditor, FormEditorError, GraphForm,
    GraphItem, GraphItemKind, SourceSelection,
};
pub use topology::{PatchbayTopology, TopologyDocument, TopologyViewError};
pub const MAX_FORM_SOURCE_BYTES: usize = conduit_form::MAXIMUM_FORM_SOURCE_BYTES;

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
        let nonce = fresh_nonce();
        Self::with_identity(
            HostId::from(format!("patchbay-native/{nonce}")),
            BootId::from(format!("patchbay-boot/{nonce}")),
        )
    }

    /// Deterministic constructor for conformance tests and embedding.
    pub fn with_identity(host_id: HostId, boot_id: BootId) -> Self {
        let host = StdHost::new_with_composition(
            StdHostConfig {
                host_id,
                boot_id,
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::minimal(),
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
            evidence_id: EvidenceId::from(format!(
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
            links: Vec::new(),
            plans: Vec::new(),
            plays: Vec::new(),
            retention: RetentionReport {
                item_capacity: LIFECYCLE_CAPACITY,
                retained_items: observations.len() as u32,
                dropped_items: 0,
            },
            observations,
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
        assert!(advertised.is_empty());
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
