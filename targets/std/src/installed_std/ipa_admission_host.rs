//! Exact per-placement authored corpus custody and prepared bounded IPA outcomes.
use conduit_core::{ConfigurationValue, PlanFragment, PlannedGear};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    ipa_authoring::{phonemic_from_ipa, phonetic_from_ipa},
    ipa_contract::*,
    semantic::*,
};
use sha2::{Digest, Sha256};
enum RetainedRequest {
    Phonetic(Box<SpeechPhoneticIpaRequest>),
    Phonemic(Box<SpeechPhonemicIpaRequest>, Box<SpeechInventory>),
}
struct Prepared {
    placement: PlannedGear,
    digest: [u8; 32],
    request: RetainedRequest,
    outcome: Vec<u8>,
}
pub(super) struct IpaAdmissionHosts(Vec<Prepared>);
fn configuration_named<'a>(placement: &'a PlannedGear, name: &str) -> Result<&'a [u8], String> {
    let entry = placement
        .configuration
        .iter()
        .find(|entry| entry.key == name)
        .ok_or("IPA missing typed startup argument")?;
    let ConfigurationValue::Structured(value) = &entry.value else {
        return Err("IPA requires typed startup argument".into());
    };
    if value.canonical_value().len() > MAXIMUM_IPA_INPUT_BYTES as usize {
        return Err("IPA startup exceeds prepared bound".into());
    }
    Ok(value.canonical_value())
}
fn configuration(placement: &PlannedGear) -> Result<&[u8], String> {
    configuration_named(placement, "request")
}
pub(super) fn validate(placement: &PlannedGear) -> Result<(), String> {
    let phonemic = match placement.kind_id.as_str() {
        PHONETIC_KIND => false,
        PHONEMIC_KIND => true,
        _ => return Err("unsupported IPA constructor".into()),
    };
    let offer = ipa_offer(phonemic);
    if placement.capability_id != offer.capability_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || !placement.authority.is_empty()
        || placement.resources.len() != 1
        || placement.resources[0].class_id.as_str() != IPA_RESOURCE
        || placement.resources[0].units != 1
        || placement.resources[0].protected.is_some()
        || placement.resources[0].compute.is_some()
        || placement.resources[0].content.is_some()
    {
        return Err("IPA placement differs from admitted Back".into());
    }
    if placement.configuration.len() != if phonemic { 2 } else { 1 } {
        return Err("IPA requires exact startup arguments".into());
    }
    configuration(placement)?;
    if phonemic {
        configuration_named(placement, "inventory")?;
    }
    Ok(())
}
pub(super) fn request_digest(placement: &PlannedGear) -> Result<[u8; 32], String> {
    validate(placement)?;
    let mut hash = Sha256::new();
    hash.update(b"conduit/ipa-prepared-constructor@1");
    for text in [
        placement.placement_id.as_str(),
        placement.kind_id.as_str(),
        placement.host_id.as_str(),
        placement.boot_id.as_str(),
    ] {
        hash.update((text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    for name in ["request", "inventory"] {
        if name == "inventory" && placement.kind_id.as_str() != PHONEMIC_KIND {
            continue;
        }
        let bytes = configuration_named(placement, name)?;
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    Ok(hash.finalize().into())
}
impl IpaAdmissionHosts {
    pub(super) fn prepare(fragment: &PlanFragment) -> Result<Self, String> {
        let mut hosts = Vec::new();
        for placement in &fragment.placements {
            if !matches!(
                placement.implementation_id.as_str(),
                PHONETIC_IMPLEMENTATION | PHONEMIC_IMPLEMENTATION
            ) {
                continue;
            }
            if hosts.len() == 16 {
                return Err("IPA constructor residence exceeds sixteen admitted owners".into());
            }
            let digest = request_digest(placement)?;
            let bytes = configuration(placement)?;
            let (request, outcome) = if placement.kind_id.as_str() == PHONETIC_KIND {
                let request = SpeechPhoneticIpaRequest::decode(bytes)
                    .map_err(|error| format!("decode IPA request: {error:?}"))?;
                let outcome = phonetic_from_ipa(&request)
                    .encode()
                    .map_err(|error| format!("encode IPA outcome: {error:?}"))?;
                (RetainedRequest::Phonetic(Box::new(request)), outcome)
            } else {
                let request = SpeechPhonemicIpaRequest::decode(bytes)
                    .map_err(|error| format!("decode phonemic request: {error:?}"))?;
                let inventory =
                    SpeechInventory::decode(configuration_named(placement, "inventory")?)
                        .map_err(|error| format!("decode IPA inventory: {error:?}"))?;
                let outcome = phonemic_from_ipa(&request, &inventory)
                    .encode()
                    .map_err(|error| format!("encode phonemic outcome: {error:?}"))?;
                (
                    RetainedRequest::Phonemic(Box::new(request), Box::new(inventory)),
                    outcome,
                )
            };
            if outcome.len() > MAXIMUM_IPA_OUTPUT_BYTES as usize {
                return Err("IPA outcome exceeds admitted bound".into());
            }
            hosts.push(Prepared {
                placement: placement.clone(),
                digest,
                request,
                outcome,
            });
        }
        Ok(Self(hosts))
    }
    pub(super) fn execute(&self, placement: &PlannedGear, input: &[u8]) -> Result<&[u8], String> {
        let prepared = self
            .0
            .iter()
            .find(|entry| entry.placement == *placement)
            .ok_or("IPA prepared placement is stale or foreign")?;
        let original = match &prepared.request {
            RetainedRequest::Phonetic(request) => request.original(),
            RetainedRequest::Phonemic(request, inventory) => {
                debug_assert!(!inventory.identity().get().is_empty());
                request.original()
            }
        };
        debug_assert!(original.len() <= 4096);
        if input != prepared.digest {
            return Err("IPA request digest differs from exact prepared arguments".into());
        }
        Ok(&prepared.outcome)
    }
}
