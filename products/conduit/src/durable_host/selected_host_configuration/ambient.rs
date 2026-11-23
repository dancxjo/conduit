//! Initialize explicitly selected Ambient equipment through the ordinary Host.
use conduit_core::AuthorityGrant;
use conduit_host_make::{CheckedHostConfiguration, HostImage};
use conduit_std_host::{
    ambient_home_observation::CheckedAmbientHomeSource,
    ambient_wallpaper::acquisition::prepare_current_public_deck, StdHost, StdHostConfig,
};
use conduit_std_offers::{
    ambient_home_source_offer, AMBIENT_HOME_SOURCE_AUTHORITY, AMBIENT_HOME_SOURCE_CALL,
};

pub(super) fn initialize(
    config: StdHostConfig,
    image: &HostImage,
    checked: &CheckedHostConfiguration,
) -> Result<StdHost, String> {
    let (grant, acquire_photos) = admitted_provider(&config, checked)?;
    // The locally selected checked configuration explicitly admits the exact
    // worker endpoint and photo source. The Host reserves their bounded backing
    // before invoking the cooperative worker factory.
    unsafe {
        StdHost::from_selected_ambient_home_image(
            config,
            image,
            &conduit_workspace_make::catalog(),
            checked,
            grant,
            || Ok(prepare_photos(acquire_photos)),
        )
    }
}

fn admitted_provider(
    config: &StdHostConfig,
    checked: &CheckedHostConfiguration,
) -> Result<(AuthorityGrant, bool), String> {
    let source = CheckedAmbientHomeSource::from_configuration(checked)?;
    let offer = ambient_home_source_offer()?;
    let grant = AuthorityGrant {
        grant_id: format!("{}/ambient-provider", config.boot_id.as_str()).into(),
        contract_id: AMBIENT_HOME_SOURCE_AUTHORITY.into(),
        host_call_contract_id: AMBIENT_HOME_SOURCE_CALL.into(),
        subject_kind: offer.kind_id,
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: offer.capability_id,
    };
    Ok((grant, source.acquire_nasa_photos))
}

fn prepare_photos(
    acquire: bool,
) -> Option<conduit_std_host::ambient_wallpaper::acquisition::PreparedDeck> {
    if !acquire {
        return None;
    }
    match prepare_current_public_deck() {
        Ok(deck) => Some(deck),
        Err(error) => {
            eprintln!("Ambient photo preparation unavailable: {error:?}");
            None
        }
    }
}

#[cfg(all(target_os = "linux", feature = "native-display"))]
pub(super) fn initialize_with_display(
    config: StdHostConfig,
    image: &HostImage,
    checked: &CheckedHostConfiguration,
    parameters: conduit_workspace_make::CheckedNativeMaskInitialization,
    output: &mut conduit_linux_display::Scanout<'_>,
    playback: Option<&conduit_std_host::hosted_audio::HostedPlaybackSelection>,
) -> Result<StdHost, String> {
    let (grant, acquire_photos) = admitted_provider(&config, checked)?;
    // The same checked local selection admits provider access in both paths.
    unsafe {
        StdHost::from_selected_ambient_home_and_native_mask_image(
            config,
            image,
            &conduit_workspace_make::catalog(),
            checked,
            parameters,
            output,
            playback,
            grant,
            || Ok(prepare_photos(acquire_photos)),
        )
    }
}
