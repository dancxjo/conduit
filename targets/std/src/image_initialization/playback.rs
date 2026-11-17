//! Checked speaker ownership staged on the final original IMAGE counter.
use super::*;
pub(super) fn prepare_playback<'a>(
    config: &'a StdHostConfig,
    image: &'a HostImage,
    catalog: &MakeCatalog,
    checked: &conduit_host_make::CheckedHostConfiguration,
    selected: &crate::hosted_audio::HostedPlaybackSelection,
) -> Result<
    (
        (
            crate::hosted_audio::HostedPlaybackSelection,
            crate::kernel_preparation::ImageMemoryLease,
        ),
        crate::kernel_preparation::StagedImageMemory<'a>,
    ),
    String,
> {
    let memory =
        crate::kernel_preparation::StagedImageMemory::verified(config, image, checked, catalog)?;
    let owned = prepare_playback_on(config, image, catalog, checked, selected, &memory)?;
    Ok((owned, memory))
}

pub(crate) fn prepare_playback_on(
    config: &StdHostConfig,
    image: &HostImage,
    catalog: &MakeCatalog,
    checked: &conduit_host_make::CheckedHostConfiguration,
    selected: &crate::hosted_audio::HostedPlaybackSelection,
    memory: &crate::kernel_preparation::StagedImageMemory<'_>,
) -> Result<
    (
        crate::hosted_audio::HostedPlaybackSelection,
        crate::kernel_preparation::ImageMemoryLease,
    ),
    String,
> {
    use conduit_host_make::ConfigurationValue;
    let mut declared = checked
        .configuration()
        .bases
        .iter()
        .filter(|b| b.kind == "audio/speaker");
    let declared = declared
        .next()
        .filter(|_| declared.next().is_none())
        .ok_or("expected one checked speaker Base")?;
    if !checked
        .resolved_bases()
        .iter()
        .any(|(kind, implementation)| {
            kind == "audio/speaker"
                && implementation == conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION
        })
    {
        return Err("speaker implementation differs from checked Source".into());
    }
    let contract = catalog
        .initialization_contracts
        .get(conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION)
        .ok_or("speaker initialization contract missing")?;
    let values = declared
        .initialization
        .as_ref()
        .ok_or("speaker initialization missing")?
        .checked_values(contract)?;
    let text = |key: &str| match values.get(key) {
        Some(ConfigurationValue::Text(value)) => Ok(value.as_str()),
        _ => Err("invalid speaker text field".to_string()),
    };
    let device = match values.get("device") {
        Some(ConfigurationValue::U64(value)) => *value,
        _ => return Err("invalid speaker device".into()),
    };
    let digest = text("executable_sha256")?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("speaker requires lowercase SHA-256".into());
    }
    let mut expected_digest = [0u8; 32];
    for (index, byte) in expected_digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&digest[index * 2..index * 2 + 2], 16)
            .map_err(|_| "invalid speaker digest")?;
    }
    if selected.boot_id != config.boot_id
        || selected.offer_generation != config.offer_generation
        || selected.observation.card_id != text("card_id")?
        || selected.observation.base_identity != text("base_identity")?
        || u64::from(selected.observation.device) != device
        || selected.executable_digest() != Some(expected_digest)
    {
        return Err("observed speaker differs from checked Source/Boot".into());
    }
    let mut bases = image.manifest.base_selections.iter().filter(|b| {
        b.kind == "audio/speaker"
            && b.driver == conduit_std_offers::AUDIO_PLAY_ALSA_HW_IMPLEMENTATION
    });
    let base = bases
        .next()
        .filter(|_| bases.next().is_none())
        .ok_or("expected one IMAGE speaker Base")?;
    let pool = selected.pool_id();
    let mut budgets = image
        .manifest
        .resource_budgets
        .iter()
        .filter(|b| b.id == pool.as_str());
    let budget = budgets
        .next()
        .filter(|_| budgets.next().is_none())
        .ok_or("speaker requires its exact physical pool budget")?;
    if budget.class != conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS
        || budget.slots != 1
        || budget.bytes == 0
    {
        return Err("speaker pool differs from actual finite equipment capacity".into());
    }
    // Borrowed input stores are independently owned. Reserve before cloning;
    // cloned String backing is bounded by these already-retained capacities.
    let bytes = selected
        .backing_bytes()
        .checked_add(base.id.len())
        .and_then(|n| {
            n.checked_add(crate::kernel_preparation::ImageMemoryLease::owned_backing_bytes())
        })
        .ok_or("speaker backing overflow")?;
    let mut lease = memory.reserve(bytes)?;
    let mut owned = selected.clone();
    owned.bind_source_base(&base.id);
    lease.retain_result(
        owned.backing_bytes() + crate::kernel_preparation::ImageMemoryLease::owned_backing_bytes(),
    );
    Ok((owned, lease))
}
