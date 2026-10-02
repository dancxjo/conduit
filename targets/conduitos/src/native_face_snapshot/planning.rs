//! Ordinary checked authoring and exact Host offer for a pure snapshot relay.
use super::*;
use alloc::{collections::BTreeMap, format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, ConnectionTrack, HostAdvertisement, HostProfileId,
    ImplementationId, ImplementationOffer, PROTOCOL_VERSION, PortDirection, port_id,
};
use conduit_planner::{
    ConnectionQueueLimits, ForeBoundaryKey, PlanningOptions, default_expanded_placements,
    plan_expanded_authoring_with_options,
};
use conduit_plot::{
    KindSignature, ProfileCatalog, StartupCatalog, check_syntax_document,
    expand_canonical_plot_for_authoring, parse_syntax_document,
};
use conduit_presentation::{
    install_mask_plot_value_aliases, presentation_tee_kind_projection, presentation_tee_offer,
};

const SOURCE: &str = include_str!("../../../../plots/face-snapshot/main.conduit");
pub(super) const IMPLEMENTATION: &str = "conduitos/presentation-snapshot-forward@1";

impl NativeFaceSnapshotProducer {
    /// The running Host offers the exact pure Back implemented by this module.
    /// Planning allocates finite storage and grants no platform-effect authority.
    pub fn prepare(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        build_id: &str,
    ) -> Result<Self, FaceSnapshotRefusal> {
        if [host_id.as_str(), boot_id.as_str(), build_id]
            .iter()
            .any(|value| value.is_empty() || value.len() > 256)
        {
            return Err(FaceSnapshotRefusal::HostIdentity);
        }
        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        install_mask_plot_value_aliases(&mut startup).map_err(|_| FaceSnapshotRefusal::Catalog)?;
        let tee = presentation_tee_kind_projection();
        startup
            .insert(KindSignature {
                kind: tee.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .map_err(|_| FaceSnapshotRefusal::Catalog)?;
        profiles
            .insert(tee)
            .map_err(|_| FaceSnapshotRefusal::Catalog)?;
        let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
            .map_err(|_| FaceSnapshotRefusal::Catalog)?;
        let authoring =
            expand_canonical_plot_for_authoring(&checked, "host-face-snapshot", &profiles)
                .map_err(|_| FaceSnapshotRefusal::Catalog)?;
        let host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id,
            boot_id,
            offer_generation,
            profile: HostProfileId::from("conduitos/host-face-snapshot@1"),
            bases: Vec::new(),
            resources: Vec::new(),
            planner_capabilities: Vec::new(),
            capabilities: vec![presentation_tee_offer(
                CapabilityId::from("conduitos/presentation-snapshot-forward"),
                ImplementationOffer {
                    implementation_id: ImplementationId::from(IMPLEMENTATION),
                    artifact_id: ArtifactId::from(format!("{IMPLEMENTATION}/{build_id}")),
                    execution_profile_id: "conduitos/bounded-face-snapshot@1".into(),
                },
                CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: MAX_SNAPSHOT_BYTES as u32,
                },
            )],
        };
        let placements =
            default_expanded_placements(&authoring.expanded, core::slice::from_ref(&host))
                .map_err(|_| FaceSnapshotRefusal::Plan)?;
        let boundary_limits = [
            (PortDirection::Input, "snapshot"),
            (PortDirection::Output, "face"),
        ]
        .into_iter()
        .map(|(direction, name)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: port_id(name),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: MAX_SNAPSHOT_BYTES as u32,
                },
            )
        })
        .collect();
        let plan = plan_expanded_authoring_with_options(
            &authoring,
            &[host],
            &placements,
            &[],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: MAX_SNAPSHOT_BYTES as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundary_limits,
        )
        .map_err(|_| FaceSnapshotRefusal::Plan)?;
        if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
            return Err(FaceSnapshotRefusal::Plan);
        }
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0])
            .map_err(|_| FaceSnapshotRefusal::Plan)?;
        execution::validate_shape(&plan, &lowered)?;
        Ok(Self {
            plan,
            lowered,
            last_play_sequence: None,
        })
    }
}
