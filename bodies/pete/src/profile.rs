use conduit_form::{ProfileCatalog, StartupCatalog};

pub fn catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_semantic_catalog::install_robotics_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_state_toggle_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profile)?;
    Ok((startup, profile))
}
