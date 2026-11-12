//! Stopped local OS policy for one immutable Text generation. Network input and
//! advertised needs cannot select its directory, resource or operation grant.
use super::*;
use base64::Engine;
use conduit_core::*;
use conduit_host_make::{CheckedHostConfiguration, HostImage};
use conduit_std_host::{NativeTextFileDefinition, StdHostConfig, TextFileInitialization};
use conduit_std_offers::*;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    pub directory: PathBuf,
    /// Canonical portable DataRef encoding, not a path or content hash shortcut.
    pub reference_base64: String,
}

pub(crate) fn prepare(
    selected: &Selection,
    config: &StdHostConfig,
    image: &HostImage,
    checked: &CheckedHostConfiguration,
) -> Result<TextFileInitialization, String> {
    let intent =
        conduit_workspace_make::CheckedTextFileInitialization::from_configuration(checked)?;
    let maximum_reference = conduit_data::maximum_data_reference_encoded_bytes("value/text")
        .ok_or("Text reference has no finite encoding bound")?;
    if !selected.directory.is_absolute()
        || selected.reference_base64.len() > maximum_reference.div_ceil(3) * 4
    {
        return Err("Text file policy requires an absolute directory and finite reference".into());
    }
    let mut encoded = [0; MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES];
    let length = base64::engine::general_purpose::STANDARD
        .decode_slice(&selected.reference_base64, &mut encoded)
        .map_err(|_| "Text file policy reference is not finite Base64")?;
    let reference =
        conduit_data::DataReference::decode_for(&kind_id("value/text"), &encoded[..length])
            .map_err(|e| format!("Text file policy reference: {e:?}"))?
            .reference()
            .clone();
    if reference.extent.bytes > u64::from(intent.maximum_text_bytes) {
        return Err("Text reference exceeds checked residence extent".into());
    }
    let mut bases = image
        .manifest
        .base_selections
        .iter()
        .filter(|base| base.kind == DURABLE_TEXT_BASE_KIND && base.driver == intent.driver);
    let base = bases
        .next()
        .ok_or("IMAGE lacks the checked Text provider")?;
    if bases.next().is_some() {
        return Err("IMAGE has ambiguous Text providers".into());
    }
    let mut pools = image
        .manifest
        .resource_budgets
        .iter()
        .filter(|pool| pool.class == DURABLE_TEXT_RESOURCE);
    let pool = pools
        .next()
        .ok_or("IMAGE lacks the exact Text residence pool")?;
    if pools.next().is_some()
        || pool.slots != 1
        || pool.bytes < u64::from(intent.maximum_text_bytes)
    {
        return Err("Text file policy requires one exact checked generation pool".into());
    }
    let content = ResourceContentRequirement {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        maximum_bytes: intent.maximum_text_bytes,
        maximum_items: 1,
        retention: ResourceRetention::ExternalDurable,
        sharing: ResourceSharing::SingleWriterPublished,
        access: if intent.write {
            ResourceAccessMode::WriteCandidatePublish
        } else {
            ResourceAccessMode::ReadPublished
        },
        generation_slots: 1,
        reader_leases: 1,
        publication_slots: u16::from(intent.write),
        sensitive: false,
    };
    let offer = durable_text_offer(content.clone()).map_err(str::to_owned)?;
    // The independently installed local policy selects this actual operation
    // and directory. This grant is not copied from an advertisement or Plan.
    let authority = AuthorityGrant {
        grant_id: fresh_identity("grant/local-text-file", &selected.reference_base64)
            .as_str()
            .into(),
        contract_id: DURABLE_TEXT_AUTHORITY.into(),
        host_call_contract_id: if intent.write {
            DURABLE_TEXT_PUBLISH_CALL
        } else {
            DURABLE_TEXT_READ_CALL
        }
        .into(),
        subject_kind: if intent.write {
            DURABLE_TEXT_SAVE_KIND
        } else {
            DURABLE_TEXT_LOAD_KIND
        }
        .into(),
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: intent.driver.as_str().into(),
    };
    let base_id: HostBaseId = base.id.as_str().into();
    Ok(TextFileInitialization {
        definition: NativeTextFileDefinition {
            base: BaseProviderEntry {
                base_id: base_id.clone(),
                provider_instance_id: format!("{}/text-file", config.host_id.as_str())
                    .as_str()
                    .into(),
                provider_generation: config.offer_generation.0,
                implementation_id: intent.driver.as_str().into(),
                mechanism_family: DURABLE_TEXT_BASE_KIND.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![ResourceOffer {
                    pool_id: pool.id.as_str().into(),
                    class_id: DURABLE_TEXT_RESOURCE.into(),
                    capacity_units: 1,
                    compute: None,
                    content: Some(ResourceContentOffer {
                        contract: content,
                        owner_host: config.host_id.clone(),
                        owner_boot: config.boot_id.clone(),
                        base_id,
                        residence_profile: DURABLE_TEXT_RESIDENCE.into(),
                    }),
                }],
            },
            authority,
            maximum_text_bytes: intent.maximum_text_bytes,
            preparation_bytes: intent
                .preparation_bytes
                .try_into()
                .map_err(|_| "Text preparation overflow")?,
            retained_bytes: intent
                .retained_bytes
                .try_into()
                .map_err(|_| "Text retention overflow")?,
        },
        directory: fs::File::open(&selected.directory)
            .map_err(|e| format!("open owner-selected Text directory: {e}"))?,
        reference,
    })
}

#[cfg(test)]
pub(crate) use tests::fixture as text_file_fixture;
#[cfg(test)]
#[path = "text_file_policy_tests.rs"]
mod tests;
