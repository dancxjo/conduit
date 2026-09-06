//! Optional browser snapshot operations; residence and authority come from Plan bindings.
use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use crate::resource_snapshot::*;
use conduit_core::*;
use conduit_kernel::HostedValueStore;

const WRITE: &str = "browser/resource-json-publish@1";
const READ: &str = "browser/resource-json-read@1";
static PUBLISH: BrowserInstallation = BrowserInstallation {
    implementation_id: WRITE,
    offer: publish_offer,
    prepare,
    perform: None,
};
static RESTORE: BrowserInstallation = BrowserInstallation {
    implementation_id: READ,
    offer: read_offer,
    prepare,
    perform: None,
};
pub(super) fn factory(id: &str) -> Option<&'static BrowserInstallation> {
    match id {
        WRITE => Some(&PUBLISH),
        READ => Some(&RESTORE),
        _ => None,
    }
}
fn publish_offer() -> CapabilityOffer {
    base_offer(true)
}
fn read_offer() -> CapabilityOffer {
    base_offer(false)
}
fn base_offer(publish: bool) -> CapabilityOffer {
    let contract = conduit_semantic_catalog::resource_snapshot_contract(publish);
    let kind = contract.kind_id.clone();
    let implementation = if publish { WRITE } else { READ };
    let operation = if publish {
        PUBLISH_OPERATION
    } else {
        READ_OPERATION
    };
    let mut offer = conduit_semantic_catalog::realization_offer(
        contract,
        conduit_semantic_catalog::SNAPSHOT_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: implementation,
            execution_profile: "browser/resource-json@1",
            implementation,
            artifact: "conduit-browser-runtime/resource-json@1",
        },
        vec![HostOperationRequirement {
            contract_id: operation.into(),
            target_kind: Some(kind.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: if publish { 4096 } else { 512 },
            maximum_output_bytes: if publish { 512 } else { 4096 },
        }],
        Vec::new(),
        vec![AuthorityRequirement {
            contract_id: AUTHORITY_CONTRACT.into(),
            host_operation_contract_id: operation.into(),
            subject_kind: kind,
        }],
    );
    offer.startup_parameters[0].has_default = false;
    offer
}

/// Select one exact local durable generation into the Host's planning surface.
/// This does not grant authority; callers must supply the separately admitted grant.
pub fn advertisement(
    host: HostId,
    boot: BootId,
    resource: ResourceOffer,
) -> Result<HostAdvertisement, String> {
    let content = resource
        .content
        .as_ref()
        .ok_or("snapshot resource has no content contract")?;
    content.validate().map_err(|error| format!("{error:?}"))?;
    if content.owner_host != host
        || content.owner_boot != boot
        || content.contract.retention != ResourceRetention::ExternalDurable
    {
        return Err("snapshot resource has the wrong residence or lifetime".into());
    }
    let mut offer =
        base_offer(content.contract.access == ResourceAccessMode::WriteCandidatePublish);
    offer.resource_requirements.push(ResourceRequirement {
        class_id: resource.class_id.clone(),
        units: 1,
        compute: None,
        protected_role: None,
        content: Some(content.contract.clone()),
    });
    let mut host = super::advertisement(host, boot);
    host.resources.push(resource);
    host.capabilities.push(offer);
    host.planner_capabilities[0].limits.maximum_authority_grants = 1;
    Ok(host)
}

pub(crate) fn reference(placement: &PlannedGear) -> Result<BoundedResourceRef, String> {
    let [configuration] = placement.configuration.as_slice() else {
        return Err("snapshot requires one reference".into());
    };
    let ConfigurationValue::Text(hex) = &configuration.value else {
        return Err("snapshot reference is not text".into());
    };
    if configuration.key != "reference" || hex.is_empty() || hex.len() > 1024 || hex.len() % 2 != 0
    {
        return Err("snapshot reference encoding exceeds its bound".into());
    }
    let bytes = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |byte| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                _ => None,
            };
            Ok((digit(pair[0]).ok_or("noncanonical reference hex")? << 4)
                | digit(pair[1]).ok_or("noncanonical reference hex")?)
        })
        .collect::<Result<Vec<u8>, &str>>()?;
    let reference = BoundedResourceRef::decode(&bytes).map_err(|error| format!("{error:?}"))?;
    if reference.content_profile.as_str() != conduit_web::JSON_TEXT_INFO_ID {
        return Err("snapshot reference has the wrong content profile".into());
    }
    Ok(reference)
}
fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    let publish = placement.implementation_id.as_str() == WRITE;
    validate_placement(placement, &base_offer(publish))?;
    PreparedSnapshotRecord::prepare(placement, &reference(placement)?)
        .map_err(|error| format!("{error:?}"))?;
    Ok(BrowserOperation::unary(if publish { 4096 } else { 512 }, 1))
}
