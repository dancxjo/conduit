use std::collections::{BTreeMap, BTreeSet};

use conduit_core::{
    BootId, CapabilityOffer, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PlannerCapabilityOffer, ResourceOffer, SignId, PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};

use crate::{
    verify_image_binding, BuildDiagnostic, BuildManifest, FabricationCatalog, HostImage,
    PrerequisiteNode,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeFacts {
    pub ready_resource_classes: BTreeSet<String>,
    pub initialized_base_kinds: BTreeSet<String>,
    pub initialized_driver_kinds: BTreeSet<String>,
    pub available_facilities: BTreeSet<String>,
    pub authority_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageBootIdentity {
    pub profile_id: String,
    pub build_id: String,
    pub image_id: String,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub offer_sign_id: SignId,
    pub inclusion_paths: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundHostAdvertisement {
    identity: ImageBootIdentity,
    advertisement: HostAdvertisement,
}

impl BoundHostAdvertisement {
    pub fn identity(&self) -> &ImageBootIdentity {
        &self.identity
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }

    pub fn into_parts(self) -> (ImageBootIdentity, HostAdvertisement) {
        (self.identity, self.advertisement)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeBindingDiagnostic {
    Image(BuildDiagnostic),
    IdentityMismatch {
        field: &'static str,
    },
    UnexpectedImplementation {
        implementation: String,
    },
    UnexpectedResourceClass {
        class: String,
    },
    ResourceCapacityExceeded {
        class: String,
        offered: u64,
        built: u64,
    },
}

#[derive(Debug, Clone)]
pub struct RuntimeOfferInputs {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    /// The admitted observation Sign that proves these runtime facts for this generation.
    pub offer_sign_id: SignId,
    pub host_profile: HostProfileId,
    pub candidate_resources: Vec<ResourceOffer>,
    pub candidate_capabilities: Vec<CapabilityOffer>,
    pub planner_capabilities: Vec<PlannerCapabilityOffer>,
    pub facts: RuntimeFacts,
}

pub fn bind_runtime_offer(
    manifest: &BuildManifest,
    image: &HostImage,
    image_bytes: &[u8],
    catalog: &FabricationCatalog,
    inputs: RuntimeOfferInputs,
) -> Result<BoundHostAdvertisement, RuntimeBindingDiagnostic> {
    verify_image_binding(image, image_bytes).map_err(RuntimeBindingDiagnostic::Image)?;
    verify_manifest_image(manifest, image)?;

    let resources = inputs
        .candidate_resources
        .into_iter()
        .filter(|offer| resource_is_ready(manifest, offer, &inputs.facts))
        .collect::<Vec<_>>();
    let capabilities = inputs
        .candidate_capabilities
        .into_iter()
        .filter(|offer| capability_is_ready(manifest, catalog, offer, &resources, &inputs.facts))
        .collect::<Vec<_>>();
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: inputs.host_id.clone(),
        boot_id: inputs.boot_id.clone(),
        offer_generation: inputs.offer_generation,
        profile: inputs.host_profile,
        resources,
        capabilities,
        planner_capabilities: inputs.planner_capabilities,
    };
    let identity = ImageBootIdentity {
        profile_id: manifest.profile_id.clone(),
        build_id: manifest.build_id.clone(),
        image_id: manifest.image_id.clone(),
        host_id: inputs.host_id,
        boot_id: inputs.boot_id,
        offer_generation: inputs.offer_generation,
        offer_sign_id: inputs.offer_sign_id,
        inclusion_paths: manifest.inclusion_paths.clone(),
    };
    let bound = BoundHostAdvertisement {
        identity,
        advertisement,
    };
    verify_bound_advertisement(&bound, manifest, image, image_bytes)?;
    Ok(bound)
}

pub fn verify_bound_advertisement(
    bound: &BoundHostAdvertisement,
    manifest: &BuildManifest,
    image: &HostImage,
    image_bytes: &[u8],
) -> Result<(), RuntimeBindingDiagnostic> {
    verify_runtime_advertisement(
        &bound.identity,
        &bound.advertisement,
        manifest,
        image,
        image_bytes,
    )
}

pub fn verify_runtime_advertisement(
    identity: &ImageBootIdentity,
    advertisement: &HostAdvertisement,
    manifest: &BuildManifest,
    image: &HostImage,
    image_bytes: &[u8],
) -> Result<(), RuntimeBindingDiagnostic> {
    verify_image_binding(image, image_bytes).map_err(RuntimeBindingDiagnostic::Image)?;
    verify_manifest_image(manifest, image)?;
    if identity.profile_id != manifest.profile_id {
        return Err(RuntimeBindingDiagnostic::IdentityMismatch {
            field: "profile_id",
        });
    }
    if identity.build_id != manifest.build_id {
        return Err(RuntimeBindingDiagnostic::IdentityMismatch { field: "build_id" });
    }
    if identity.image_id != manifest.image_id {
        return Err(RuntimeBindingDiagnostic::IdentityMismatch { field: "image_id" });
    }
    if identity.host_id != advertisement.host_id {
        return Err(RuntimeBindingDiagnostic::IdentityMismatch { field: "host_id" });
    }
    if identity.boot_id != advertisement.boot_id {
        return Err(RuntimeBindingDiagnostic::IdentityMismatch { field: "boot_id" });
    }
    if identity.offer_generation != advertisement.offer_generation {
        return Err(RuntimeBindingDiagnostic::IdentityMismatch {
            field: "offer_generation",
        });
    }
    for offer in &advertisement.capabilities {
        let implementation = offer.implementation.implementation_id.as_str();
        if !manifest
            .implementations
            .iter()
            .any(|item| item == implementation)
            && !manifest
                .presenters
                .iter()
                .any(|item| item == implementation)
        {
            return Err(RuntimeBindingDiagnostic::UnexpectedImplementation {
                implementation: implementation.into(),
            });
        }
    }
    for offer in &advertisement.resources {
        validate_resource_offer(manifest, offer)?;
    }
    Ok(())
}

fn verify_manifest_image(
    manifest: &BuildManifest,
    image: &HostImage,
) -> Result<(), RuntimeBindingDiagnostic> {
    for (field, matches) in [
        (
            "profile_id",
            manifest.profile_id == image.payload.profile_id,
        ),
        ("build_id", manifest.build_id == image.payload.build_id),
        ("image_id", manifest.image_id == image.manifest.image_id),
    ] {
        if !matches {
            return Err(RuntimeBindingDiagnostic::IdentityMismatch { field });
        }
    }
    Ok(())
}

fn resource_is_ready(
    manifest: &BuildManifest,
    offer: &ResourceOffer,
    facts: &RuntimeFacts,
) -> bool {
    facts
        .ready_resource_classes
        .contains(offer.class_id.as_str())
        && validate_resource_offer(manifest, offer).is_ok()
}

fn validate_resource_offer(
    manifest: &BuildManifest,
    offer: &ResourceOffer,
) -> Result<(), RuntimeBindingDiagnostic> {
    let class = offer.class_id.as_str();
    let budgets = manifest
        .resource_budgets
        .iter()
        .filter(|budget| budget.class == class)
        .collect::<Vec<_>>();
    if budgets.is_empty() {
        return Err(RuntimeBindingDiagnostic::UnexpectedResourceClass {
            class: class.into(),
        });
    }
    let built = budgets
        .iter()
        .map(|budget| u64::from(budget.slots))
        .sum::<u64>();
    if u64::from(offer.capacity_units) > built {
        return Err(RuntimeBindingDiagnostic::ResourceCapacityExceeded {
            class: class.into(),
            offered: u64::from(offer.capacity_units),
            built,
        });
    }
    Ok(())
}

fn capability_is_ready(
    manifest: &BuildManifest,
    catalog: &FabricationCatalog,
    offer: &CapabilityOffer,
    resources: &[ResourceOffer],
    facts: &RuntimeFacts,
) -> bool {
    let implementation = offer.implementation.implementation_id.as_str();
    let built_implementation = manifest
        .implementations
        .iter()
        .any(|item| item == implementation);
    let built_presenter = manifest
        .presenters
        .iter()
        .any(|item| item == implementation);
    if (!built_implementation && !built_presenter)
        || offer.host_operations.iter().any(|requirement| {
            !manifest
                .host_operations
                .iter()
                .any(|item| item == requirement.contract_id.as_str())
        })
        || offer.resource_requirements.iter().any(|requirement| {
            !resources.iter().any(|resource| {
                resource.class_id == requirement.class_id
                    && resource.capacity_units >= requirement.units
            })
        })
        || (!facts.authority_ready && !offer.authority_requirements.is_empty())
    {
        return false;
    }
    let prerequisites = catalog
        .implementations
        .get(implementation)
        .map(|metadata| &metadata.prerequisites)
        .or_else(|| {
            catalog
                .presenters
                .get(implementation)
                .map(|metadata| &metadata.prerequisites)
        });
    prerequisites.is_some_and(|nodes| {
        nodes
            .iter()
            .all(|node| runtime_node_ready(manifest, catalog, node, facts, &mut BTreeSet::new()))
    })
}

fn runtime_node_ready(
    manifest: &BuildManifest,
    catalog: &FabricationCatalog,
    node: &PrerequisiteNode,
    facts: &RuntimeFacts,
    visiting: &mut BTreeSet<PrerequisiteNode>,
) -> bool {
    if !visiting.insert(node.clone()) {
        return false;
    }
    let ready = match node {
        PrerequisiteNode::Implementation(value) => manifest.implementations.contains(value),
        PrerequisiteNode::HostOperation(value) => manifest.host_operations.contains(value),
        PrerequisiteNode::Resource(value) => facts.ready_resource_classes.contains(value),
        PrerequisiteNode::Base(value) => {
            manifest
                .base_selections
                .iter()
                .any(|item| &item.kind == value)
                && facts.initialized_base_kinds.contains(value)
        }
        PrerequisiteNode::Driver(value) => {
            manifest
                .driver_selections
                .iter()
                .any(|item| &item.kind == value)
                && facts.initialized_driver_kinds.contains(value)
        }
        PrerequisiteNode::Facility(value) => {
            manifest.facilities.contains(value) && facts.available_facilities.contains(value)
        }
    } && catalog.dependencies.get(node).is_none_or(|dependencies| {
        dependencies
            .iter()
            .all(|dependency| runtime_node_ready(manifest, catalog, dependency, facts, visiting))
    });
    visiting.remove(node);
    ready
}
