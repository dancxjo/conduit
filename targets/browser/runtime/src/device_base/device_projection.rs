//! Optional Device context derived only from current acquired resource truth.

use conduit_core::{
    CapabilityId, DeviceAssociation, DeviceId, DeviceIdentityEvidence, DeviceIdentityFact,
    DeviceIdentityStrength, DeviceResourceProvenance, DeviceTruthDisposition, PROTOCOL_VERSION,
};

use super::BrowserSerialPhase;

pub(super) fn current_device_association(
    phase: &BrowserSerialPhase,
    mut capability_ids: Vec<CapabilityId>,
) -> Option<DeviceAssociation> {
    let resource = match phase {
        BrowserSerialPhase::ResourceTruth(resource)
        | BrowserSerialPhase::UsePlanned { resource, .. }
        | BrowserSerialPhase::UsePlaying { resource, .. } => resource,
        _ => return None,
    };
    capability_ids.sort();
    capability_ids.dedup();
    if capability_ids.is_empty() {
        return None;
    }
    let mut facts = Vec::new();
    if let Some(product_id) = resource.usb_product_id {
        facts.push(DeviceIdentityFact {
            name: "usb-product-id".into(),
            value: format!("{product_id:04x}"),
        });
    }
    if let Some(vendor_id) = resource.usb_vendor_id {
        facts.push(DeviceIdentityFact {
            name: "usb-vendor-id".into(),
            value: format!("{vendor_id:04x}"),
        });
    }
    Some(DeviceAssociation {
        protocol_version: PROTOCOL_VERSION,
        device_id: DeviceId::from(format!(
            "browser-serial/{}/{}",
            resource.boot_id.as_str(),
            resource.handle_id.as_str()
        )),
        host_id: resource.host_id.clone(),
        boot_id: resource.boot_id.clone(),
        offer_generation: resource.offer_generation,
        disposition: DeviceTruthDisposition::Current,
        capability_ids,
        resources: vec![DeviceResourceProvenance {
            handle_id: resource.handle_id.clone(),
            class_id: resource.class_id.clone(),
            base_implementation_id: resource.base_implementation_id.clone(),
            base_instance_id: resource.base_instance_id.clone(),
        }],
        identity_evidence: DeviceIdentityEvidence {
            strength: DeviceIdentityStrength::BootLocalResource,
            provider: resource.base_implementation_id.clone(),
            facts,
        },
    })
}
