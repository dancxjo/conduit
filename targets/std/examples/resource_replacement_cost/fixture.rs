//! One ordinary checked frame/source Gear, two truthful local storage profiles.
//! A separate target pool is explicitly reserved; payload identity is unchanged.
use conduit_core::*;
use conduit_planner::{proof::resource_frame::*, *};
use conduit_plot::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
#[path = "../../tests/resource_common/provider.rs"]
mod provider;
pub use provider::{provider, provider_with_handle};

pub fn payload() -> Vec<u8> {
    (0..FRAME_BYTES).map(|i| (i % 251) as u8).collect()
}

pub const SOURCE: &str = "plot resource_cost {\n source: frame/source\n}\n";

pub struct Cases {
    pub host: HostAdvertisement,
    pub previous: Plan,
    pub candidate: Plan,
}
impl Cases {
    pub fn new() -> Result<Self, String> {
        let seed = frame_resource_plan(false, false)?;
        let mut host = seed.host;
        host.capabilities
            .retain(|c| c.kind_id.as_str() == "frame/source");
        host.resources
            .retain(|r| r.class_id.as_str() == "resource/input-frame");
        host.host_id = format!("host/resource-cost/{}", std::process::id()).into();
        host.boot_id = format!(
            "boot/resource-cost/{}/{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        )
        .into();
        host.profile = "std/resource-cost-preparation@1".into();
        let identity = ResourceSemanticIdentity::from_digest(semantic_digest(
            "resource-cost/frame@1",
            b"synthetic-rgba-256x256/i-mod-251",
        ));
        let version = ResourceVersionIdentity::from_digest(Sha256::digest(payload()).into());
        for capability in &mut host.capabilities {
            for need in &mut capability.resource_requirements {
                if let Some(content) = &mut need.content {
                    content.identity = identity;
                    content.version = version;
                }
            }
        }
        for resource in &mut host.resources {
            if let Some(content) = &mut resource.content {
                content.contract.identity = identity;
                content.contract.version = version;
                content.owner_host = host.host_id.clone();
                content.owner_boot = host.boot_id.clone();
            }
        }
        let original = host.capabilities[0].clone();
        let mut moved = original.clone();
        moved.capability_id = "frame/source/prepared-copy-target@1".into();
        moved.implementation.implementation_id = "frame/source/prepared-copy-target@1".into();
        host.capabilities.push(moved);
        let mut target = host.resources[0].clone();
        target.pool_id = "pool/frame-moved".into();
        target.capacity_units = 1;
        target.content.as_mut().unwrap().base_id = "base/frame-target-arena".into();
        host.resources.push(target);
        host.bases = host
            .capabilities
            .iter()
            .map(|cap| {
                let pool = host
                    .resources
                    .iter()
                    .find(|r| {
                        (r.pool_id.as_str() == "pool/frame-moved")
                            != (cap.capability_id == original.capability_id)
                    })
                    .expect("one arena per capability");
                BaseProviderAdvertisement {
                    base_id: pool.content.as_ref().unwrap().base_id.clone(),
                    provider_instance_id: format!(
                        "provider/{}/{}",
                        host.boot_id.as_str(),
                        pool.pool_id.as_str()
                    )
                    .into(),
                    provider_generation: 1,
                    implementation_id: "conduit.base/local@1".into(),
                    mechanism_family: "resource/local-immutable-arena".into(),
                    enforcement_class: BaseEnforcementClass::Cooperative,
                    lifecycle: BaseLifecycle::Ready,
                    capability_ids: vec![cap.capability_id.clone()],
                    resource_pool_ids: vec![pool.pool_id.clone()],
                }
            })
            .collect();
        host.resources.sort();
        host.capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let mut startup = StartupCatalog::new();
        startup
            .insert(KindSignature {
                kind: "frame/source".into(),
                startup_parameters: vec![],
            })
            .map_err(|e| format!("{e:?}"))?;
        let mut profile = ProfileCatalog::new();
        profile
            .insert(KindProjection {
                kind_id: original.kind_id.clone(),
                kind_contract_revision: original.kind_contract_revision.clone(),
                inputs: original.inputs.clone(),
                outputs: original.outputs.clone(),
                configuration: original.semantic_contract.configuration.clone(),
            })
            .map_err(|e| format!("{e:?}"))?;
        let syntax = parse_syntax_document(SOURCE);
        let checked = check_syntax_document(&syntax, &startup).map_err(|e| format!("{e:?}"))?;
        let expanded = expand_canonical_plot(&checked, "resource_cost", &profile)
            .map_err(|e| format!("{e:?}"))?;
        let plan = |capability: CapabilityId| {
            let choices = PlacementChoices {
                by_gear: BTreeMap::from([(
                    expanded.gears[0].gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: capability,
                    },
                )]),
            };
            plan_expanded_canonical_with_options(
                &expanded,
                &[host.clone()],
                &choices,
                &[BaseImplementationId::from("conduit.base/local@1")],
                PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: 512,
                    authority_grants: &[],
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .map_err(|e| e.to_string())
        };
        let previous = plan(original.capability_id)?;
        let candidate = plan("frame/source/prepared-copy-target@1".into())?;
        if previous.plan_id == candidate.plan_id
            || previous.source_document_id != candidate.source_document_id
            || previous.checked_plot_id != candidate.checked_plot_id
            || previous.expanded_plot_id != candidate.expanded_plot_id
            || previous.fragments[0].placements[0].resources[0].pool_id
                == candidate.fragments[0].placements[0].resources[0].pool_id
        {
            return Err("resource alternatives lost identity or distinct residence".into());
        }
        Ok(Self {
            host,
            previous,
            candidate,
        })
    }
    pub fn observations(&self) -> Vec<ResourceObservation> {
        self.host
            .resources
            .iter()
            .map(|r| ResourceObservation {
                host_id: self.host.host_id.clone(),
                boot_id: self.host.boot_id.clone(),
                offer_generation: self.host.offer_generation,
                pool_id: r.pool_id.clone(),
                class_id: r.class_id.clone(),
                health: ResourceHealth::Ready,
                unreserved_units: r.capacity_units,
                utilized_units: 0,
                sign_id: format!("actual-owner/{}", r.pool_id.as_str()).into(),
            })
            .collect()
    }
}
