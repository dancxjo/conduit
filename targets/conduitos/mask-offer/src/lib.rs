//! The checked ConduitOS graphical and direct-speech Mask offers and Plans.
//! Both the native guest and its installed Body owner use these exact definitions.
#![no_std]
extern crate alloc;

use alloc::{format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, CapabilityId, CapabilityLimits, ExecutionProfileId,
    HostAdvertisement, HostCallContractId, HostCallRequirement, HostId, HostProfileId,
    ImplementationId, ImplementationOffer, OfferGeneration, PROTOCOL_VERSION, authority_grant,
    kind_id, present_authority_requirement, resource_offer, resource_requirement,
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
    FaceInteractionRealizationOffer, MAX_RENDERER_VALUE_BYTES, MaskPlot, PlannedMaskPlot,
    RendererRealizationOffer, ResourceRendererRealizationOffer, ShowResourceSourceOffer,
    face_interaction_kind_projection, face_interaction_offer, install_mask_plot_value_aliases,
    presentation_tee_kind_projection, presentation_tee_offer, renderer_kind_projection,
    renderer_offer, resource_renderer_kind, resource_renderer_offer, show_resource_source_kind,
    show_resource_source_offer,
};

pub const MASK_BYTES: usize = 48 * 1024;

#[derive(Clone)]
pub struct MaskStage {
    pub planned_mask: PlannedMaskPlot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaskOfferError {
    InvalidPlan,
}

#[derive(Clone, Copy)]
pub enum Adapter {
    Native,
    Speech,
}

pub fn prepare_stage(
    adapter: Adapter,
    host_id: &HostId,
    boot_id: &conduit_core::BootId,
    generation: u64,
    surface_provider: Option<&conduit_core::BaseProviderEntry>,
) -> Result<MaskStage, MaskOfferError> {
    let advertisement = renderer_host(adapter, host_id, boot_id, generation, surface_provider);
    prepare_stage_from_offer(adapter, &advertisement)
}

/// The owner plans against the exact current offer received and admitted from
/// the native Host. The guest uses this same function after observing its Base.
pub fn prepare_stage_from_offer(
    adapter: Adapter,
    advertisement: &HostAdvertisement,
) -> Result<MaskStage, MaskOfferError> {
    if advertisement.protocol_version != PROTOCOL_VERSION
        || advertisement.profile.as_str() != "conduitos/mask-host@1"
        || advertisement.offer_generation.0 == 0
    {
        return Err(MaskOfferError::InvalidPlan);
    }
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    install_mask_plot_value_aliases(&mut startup).map_err(|_| MaskOfferError::InvalidPlan)?;
    for projection in [
        renderer_kind_projection(),
        face_interaction_kind_projection(),
        presentation_tee_kind_projection(),
    ] {
        startup
            .insert(KindSignature {
                kind: projection.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .map_err(|_| MaskOfferError::InvalidPlan)?;
        catalog
            .insert(projection)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
    }
    for kind in [show_resource_source_kind(), resource_renderer_kind()] {
        startup
            .insert(KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .map_err(|_| MaskOfferError::InvalidPlan)?;
        catalog
            .insert_kind(kind)
            .map_err(|_| MaskOfferError::InvalidPlan)?;
    }
    let (source, entry) = match adapter {
        Adapter::Native => (
            include_str!("../../../../plots/native-graphical-mask/main.conduit"),
            "native-graphical",
        ),
        Adapter::Speech => (
            include_str!("../../../../plots/spoken-mask/main.conduit"),
            "spoken",
        ),
    };
    let checked = check_syntax_document(&parse_syntax_document(source), &startup)
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    let authoring = expand_canonical_plot_for_authoring(&checked, entry, &catalog)
        .map_err(|_| MaskOfferError::InvalidPlan)?;
    let mask = MaskPlot::admit(&authoring).map_err(|_| MaskOfferError::InvalidPlan)?;
    let placements =
        default_expanded_placements(&authoring.expanded, core::slice::from_ref(advertisement))
            .map_err(|_| MaskOfferError::InvalidPlan)?;
    let boundary_limits = [
        (conduit_core::PortDirection::Input, "face"),
        (conduit_core::PortDirection::Output, "interaction"),
        (conduit_core::PortDirection::Output, "show"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            ForeBoundaryKey {
                direction,
                front_port_id: conduit_core::port_id(name),
                track: conduit_core::ConnectionTrack::Payload,
            },
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: if name == "interaction" {
                    conduit_presentation::MAX_FACE_INTERACTION_BYTES as u32
                } else {
                    MASK_BYTES as u32
                },
            },
        )
    })
    .collect();
    let empty_connections = alloc::collections::BTreeMap::new();
    let empty_lines = alloc::collections::BTreeMap::new();
    let authority_grants = if matches!(adapter, Adapter::Native) {
        let requirement =
            present_authority_requirement(kind_id("presentation/base/conduitos-surface@1"));
        vec![authority_grant(
            "conduitos/native-mask/present",
            &requirement,
            advertisement.host_id.clone(),
            advertisement.boot_id.clone(),
            CapabilityId::from("renderer-conduitos"),
        )]
    } else {
        Vec::new()
    };
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        core::slice::from_ref(advertisement),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &empty_connections,
            line_candidates: &empty_lines,
            connection_item_capacity: 1,
            connection_byte_capacity: MASK_BYTES as u32,
            authority_grants: &authority_grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .map_err(|_| MaskOfferError::InvalidPlan)?;
    Ok(MaskStage {
        planned_mask: PlannedMaskPlot::admit(&mask, &plan)
            .map_err(|_| MaskOfferError::InvalidPlan)?,
    })
}

fn renderer_host(
    adapter: Adapter,
    host_id: &HostId,
    boot_id: &conduit_core::BootId,
    generation: u64,
    surface_provider: Option<&conduit_core::BaseProviderEntry>,
) -> HostAdvertisement {
    let (capability, implementation, artifact, target_kind, resource_class, input_resource) =
        match adapter {
            Adapter::Native => (
                "renderer-conduitos",
                "presentation/renderer-conduitos-native@1",
                "conduitos/native-compositor@1",
                "presentation/base/conduitos-surface@1",
                "conduit.resource/conduitos-surface@1",
                "conduit.resource/conduitos-human-input@1",
            ),
            Adapter::Speech => (
                "renderer-test-speech",
                "presentation/renderer-test-speech@1",
                "conduitos/test-speech@1",
                "presentation/base/test-speech@1",
                "conduit.resource/test-speech-sink@1",
                "conduit.resource/test-speech-input@1",
            ),
        };
    let mut capabilities = vec![presentation_tee_offer(
        CapabilityId::from(format!("{capability}-tee")),
        ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("conduitos/bounded-mask@1"),
            implementation_id: ImplementationId::from("conduit.presentation/tee-kernel@1"),
            artifact_id: ArtifactId::from("conduitos/presentation-tee@1"),
        },
        CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
        },
    )];
    match adapter {
        Adapter::Native => {
            capabilities.push(show_resource_source_offer(ShowResourceSourceOffer {
                capability_id: CapabilityId::from("renderer-conduitos-resource-source"),
                execution_profile_id: ExecutionProfileId::from("conduitos/bounded-mask@1"),
                implementation_id: ImplementationId::from(
                    "presentation/conduitos-surface-source@1",
                ),
                artifact_id: ArtifactId::from("conduitos/native-compositor@1"),
                resource_requirement: resource_requirement(
                    conduit_presentation::SHOW_RESOURCE_CLASS,
                    1,
                ),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
                },
            }));
            capabilities.push(resource_renderer_offer(ResourceRendererRealizationOffer {
                capability_id: CapabilityId::from(capability),
                execution_profile_id: ExecutionProfileId::from("conduitos/bounded-mask@1"),
                implementation_id: ImplementationId::from(implementation),
                artifact_id: ArtifactId::from(artifact),
                host_call: HostCallRequirement {
                    contract_id: HostCallContractId::from("conduit.host/present@1"),
                    target_kind: Some(kind_id(target_kind)),
                    maximum_in_flight: 1,
                    maximum_input_bytes: MASK_BYTES as u32,
                    maximum_output_bytes: MASK_BYTES as u32,
                },
                authority_requirement: present_authority_requirement(kind_id(target_kind)),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
                },
            }));
        }
        Adapter::Speech => capabilities.push(renderer_offer(RendererRealizationOffer {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from("conduitos/bounded-mask@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_call: HostCallRequirement {
                contract_id: HostCallContractId::from("conduit.host/present@1"),
                target_kind: Some(kind_id(target_kind)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
            },
            resource_requirement: resource_requirement(resource_class, 1),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
            },
        })),
    }
    capabilities.push(face_interaction_offer(FaceInteractionRealizationOffer {
        capability_id: CapabilityId::from(format!("{capability}-input")),
        execution_profile_id: ExecutionProfileId::from("conduitos/bounded-interaction@1"),
        implementation_id: ImplementationId::from(match adapter {
            Adapter::Native => "presentation/conduitos-human-input@1",
            Adapter::Speech => "presentation/test-speech-input@1",
        }),
        artifact_id: ArtifactId::from(match adapter {
            Adapter::Native => "conduitos/native-compositor-input@1",
            Adapter::Speech => "conduitos/test-speech-input@1",
        }),
        host_call: HostCallRequirement {
            contract_id: HostCallContractId::from("conduit.host/presentation-interaction@1"),
            target_kind: Some(kind_id(conduit_presentation::FACE_INTERACTION_VALUE_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
            maximum_output_bytes: conduit_presentation::MAX_FACE_INTERACTION_BYTES as u32,
        },
        resource_requirement: resource_requirement(input_resource, 1),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 8,
            max_queue_bytes: conduit_presentation::MAX_FACE_INTERACTION_BYTES as u32 * 8,
        },
    }));
    let (surface_resource, bases) = if matches!(adapter, Adapter::Native) {
        if let Some(provider) = surface_provider {
            let resource = provider
                .resources
                .iter()
                .find(|resource| {
                    resource.class_id.as_str() == conduit_presentation::SHOW_RESOURCE_CLASS
                })
                .cloned()
                .unwrap_or_else(|| {
                    resource_offer(
                        &format!("{}/{capability}", host_id.as_str()),
                        conduit_presentation::SHOW_RESOURCE_CLASS,
                        1,
                    )
                });
            let base = conduit_core::BaseProviderAdvertisement {
                base_id: provider.base_id.clone(),
                provider_instance_id: provider.provider_instance_id.clone(),
                provider_generation: provider.provider_generation,
                implementation_id: provider.implementation_id.clone(),
                mechanism_family: provider.mechanism_family.clone(),
                enforcement_class: provider.enforcement_class,
                lifecycle: provider.lifecycle,
                capability_ids: vec![CapabilityId::from("renderer-conduitos-resource-source")],
                resource_pool_ids: vec![resource.pool_id.clone()],
            };
            (resource, vec![base])
        } else {
            (
                resource_offer(
                    &format!("{}/{capability}", host_id.as_str()),
                    conduit_presentation::SHOW_RESOURCE_CLASS,
                    1,
                ),
                Vec::new(),
            )
        }
    } else {
        (
            resource_offer(
                &format!("{}/{capability}", host_id.as_str()),
                resource_class,
                1,
            ),
            Vec::new(),
        )
    };
    let mut resources = vec![
        surface_resource,
        resource_offer(
            &format!("{}/{capability}-input", host_id.as_str()),
            input_resource,
            1,
        ),
    ];
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from("conduitos/mask-host@1"),
        bases,
        resources,
        capabilities,
        planner_capabilities: Vec::new(),
    }
}

/// Exact native-mask advertisement used by boot-time Body rendezvous.
pub fn native_host_advertisement(
    host_id: &HostId,
    boot_id: &conduit_core::BootId,
    generation: u64,
    surface_provider: &conduit_core::BaseProviderEntry,
) -> HostAdvertisement {
    renderer_host(
        Adapter::Native,
        host_id,
        boot_id,
        generation,
        Some(surface_provider),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_and_native_guest_plan_the_same_observed_surface_offer() {
        let host = HostId::from("conduitos/test-host");
        let boot = conduit_core::BootId::from("conduitos/test-boot");
        let provider = conduit_core::BaseProviderEntry {
            base_id: "conduitos/test/framebuffer".into(),
            provider_instance_id: "conduitos/test/framebuffer/provider/1".into(),
            provider_generation: 1,
            implementation_id: "conduitos/framebuffer@1".into(),
            mechanism_family: "conduitos.base/framebuffer@1".into(),
            enforcement_class: conduit_core::BaseEnforcementClass::ConduitOsKernelEnforced,
            lifecycle: conduit_core::BaseLifecycle::Ready,
            capabilities: Vec::new(),
            resources: vec![resource_offer(
                "conduitos/test/framebuffer/surface",
                conduit_presentation::SHOW_RESOURCE_CLASS,
                1,
            )],
        };
        let offer = native_host_advertisement(&host, &boot, 1, &provider);
        let guest = prepare_stage(Adapter::Native, &host, &boot, 1, Some(&provider))
            .unwrap()
            .planned_mask;
        let owner = prepare_stage_from_offer(Adapter::Native, &offer)
            .unwrap()
            .planned_mask;
        assert_eq!(owner, guest);

        let mut wrong_profile = offer;
        wrong_profile.profile = "conduitos/other".into();
        assert!(prepare_stage_from_offer(Adapter::Native, &wrong_profile).is_err());
    }
}
