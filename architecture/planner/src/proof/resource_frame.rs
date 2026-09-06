//! Named frame-resource acceptance composition using ordinary checking/planning.
use crate::{default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions};
use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::*;
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ExpandedCanonicalForm,
    KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
};
pub const FRAME_SOURCE: &str = "form frames {\n source: frame/source\n compose: frame/compose\n display: frame/display\n encoder: frame/encoder\n source > compose\n compose > display\n compose > encoder\n}\n";
pub const FRAME_BYTES: u32 = 262144;
pub const FRAME_OPERATION: &str = "conduit.host/frame-resource@1";
pub const FRAME_AUTHORITY: &str = "conduit.authority/frame-resource@1";

pub struct FrameResourcePlan {
    pub expanded: ExpandedCanonicalForm,
    pub host: HostAdvertisement,
    pub plan: Plan,
}

pub fn frame_content() -> ResourceContentRequirement {
    ResourceContentRequirement {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        version: ResourceVersionIdentity::from_digest([3; 32]),
        content_profile: kind_id("image/rgba@1"),
        maximum_bytes: FRAME_BYTES,
        maximum_items: 65536,
        retention: ResourceRetention::Play,
        sharing: ResourceSharing::SingleWriterPublished,
        access: ResourceAccessMode::WriteCandidatePublish,
        generation_slots: 1,
        reader_leases: 3,
        publication_slots: 1,
        sensitive: false,
    }
}

pub fn frame_resource_plan(
    copy: bool,
    foreign_residence: bool,
) -> Result<FrameResourcePlan, String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    let mut definitions = Vec::new();
    for name in ["source", "compose", "display", "encoder"] {
        let port = |name: &str, direction| PortDescriptor {
            port_id: port_id(name),
            value_kind: kind_id(RESOURCE_REFERENCE_INFO_ID),
            direction,
            temporal: PortTemporal::Value,
        };
        let kind = format!("frame/{name}");
        let definition = KindDefinition {
            kind_id: kind_id(&kind),
            kind_contract_revision: KindContractRevision::from(format!("{kind}@1")),
            inputs: if name == "source" {
                vec![]
            } else {
                vec![port("input", PortDirection::Input)]
            },
            outputs: if name == "source" || name == "compose" {
                vec![port("output", PortDirection::Output)]
            } else {
                vec![]
            },
            configuration: vec![],
        };
        startup
            .insert(KindSignature {
                kind,
                startup_parameters: vec![],
            })
            .map_err(|e| format!("{e:?}"))?;
        profile
            .insert(definition.clone())
            .map_err(|e| format!("{e:?}"))?;
        definitions.push(definition);
    }
    let checked = check_syntax_document(&parse_syntax_document(FRAME_SOURCE), &startup)
        .map_err(|e| format!("{e:?}"))?;
    let expanded =
        expand_canonical_form(&checked, "frames", &profile).map_err(|e| format!("{e:?}"))?;
    let host_id = HostId::from("host/frame");
    let boot_id = BootId::from("boot/frame/1");
    let mut resource = resource_offer("pool/frame-output", "resource/frame", 3);
    resource.content = Some(ResourceContentOffer {
        contract: frame_content(),
        owner_host: host_id.clone(),
        owner_boot: boot_id.clone(),
        base_id: HostBaseId::from("base/frame-arena"),
        residence_profile: kind_id(if copy {
            "arena/copied-read@1"
        } else {
            "arena/shared-read@1"
        }),
    });
    let mut input_resource = resource_offer("pool/frame-input", "resource/input-frame", 2);
    let mut initial = resource.content.clone().unwrap();
    initial.contract.version = ResourceVersionIdentity::from_digest([2; 32]);
    initial.contract.sharing = ResourceSharing::ImmutableReadMany;
    initial.contract.access = ResourceAccessMode::ReadPublished;
    initial.contract.publication_slots = 0;
    input_resource.content = Some(initial);
    let capabilities = definitions
        .iter()
        .map(|definition| {
            let source = definition.kind_id.as_str() == "frame/source";
            let compose = definition.kind_id.as_str() == "frame/compose";
            let mut requirement = resource_requirement("resource/frame", 1);
            let mut contract = frame_content();
            if !compose {
                contract.access = ResourceAccessMode::ReadPublished;
                contract.publication_slots = 0;
            }
            requirement.content = Some(contract);
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from(definition.kind_id.as_str()),
                kind_id: definition.kind_id.clone(),
                kind_contract_revision: definition.kind_contract_revision.clone(),
                implementation: ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from("frame-proof@1"),
                    implementation_id: ImplementationId::from(format!(
                        "{}/{}@1",
                        definition.kind_id.as_str(),
                        if copy { "copy" } else { "shared" }
                    )),
                    artifact_id: ArtifactId::from("frame-proof-artifact@1"),
                },
                inputs: definition.inputs.clone(),
                outputs: definition.outputs.clone(),
                host_operations: if source {
                    vec![]
                } else {
                    vec![HostOperationRequirement {
                        contract_id: HostOperationContractId::from(FRAME_OPERATION),
                        target_kind: Some(definition.kind_id.clone()),
                        maximum_in_flight: 1,
                        maximum_input_bytes: 512,
                        maximum_output_bytes: 0,
                    }]
                },
                resource_requirements: {
                    let mut inputs = vec![];
                    if source || compose {
                        let mut input = resource_requirement("resource/input-frame", 1);
                        input.content =
                            Some(input_resource.content.as_ref().unwrap().contract.clone());
                        inputs.push(input);
                    }
                    if !source {
                        inputs.push(requirement);
                    }
                    if compose || (copy && !source) {
                        inputs.push(resource_requirement(
                            "resource/frame-scratch-bytes",
                            FRAME_BYTES,
                        ));
                    }
                    inputs.sort();
                    inputs
                },
                authority_requirements: if source {
                    vec![]
                } else {
                    vec![AuthorityRequirement {
                        contract_id: AuthorityContractId::from(FRAME_AUTHORITY),
                        host_operation_contract_id: HostOperationContractId::from(FRAME_OPERATION),
                        subject_kind: definition.kind_id.clone(),
                    }]
                },
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 4,
                    max_queue_bytes: 512,
                },
            }
        })
        .collect::<Vec<_>>();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("host/frame@1"),
        resources: {
            let mut pools = vec![
                input_resource,
                resource,
                resource_offer(
                    "pool/scratch",
                    "resource/frame-scratch-bytes",
                    3 * FRAME_BYTES,
                ),
            ];
            pools.sort();
            pools
        },
        capabilities,
        planner_capabilities: vec![],
    };
    let mut hosts = vec![host.clone()];
    if foreign_residence {
        let mut remote = host.clone();
        remote.host_id = HostId::from("host/remote-consumers");
        remote.boot_id = BootId::from("boot/remote-consumers/1");
        remote
            .capabilities
            .retain(|c| matches!(c.kind_id.as_str(), "frame/display" | "frame/encoder"));
        hosts[0]
            .capabilities
            .retain(|c| !matches!(c.kind_id.as_str(), "frame/display" | "frame/encoder"));
        // The remote Host cannot satisfy the exact local Resource residence.
        // No remote dereference implementation or Line is fabricated for it.
        hosts.push(remote);
    }
    let grants = hosts
        .iter()
        .flat_map(|host| {
            host.capabilities.iter().flat_map(move |c| {
                c.authority_requirements.iter().map(|r| {
                    authority_grant(
                        &format!("grant/{}", c.kind_id.as_str()),
                        r,
                        host.host_id.clone(),
                        host.boot_id.clone(),
                        c.capability_id.clone(),
                    )
                })
            })
        })
        .collect::<Vec<_>>();
    let placements = default_expanded_placements(&expanded, &hosts).map_err(|e| e.to_string())?;
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: 512,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(FrameResourcePlan {
        expanded,
        host,
        plan,
    })
}
