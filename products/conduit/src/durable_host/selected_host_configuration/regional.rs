//! A selected viewer Base has local presentation scope. It adds no remote
//! capability, equipment claim or authority to the owner's advertisement.
use super::*;
use conduit_face::TemperatureDisplayUnit;
use conduit_host_make::BaseInitialization;
use conduit_std_host::hosted_region::RegionalPreferences;

pub(super) const IMPLEMENTATION: &str = "hosted/linux-regional-preferences@1";

pub(super) fn parameters(
    init: &BaseInitialization,
) -> Result<Option<TemperatureDisplayUnit>, String> {
    if init.values.len() != 1 {
        return Err("regional Base requires one temperature selection".into());
    }
    match init
        .values
        .get("temperature")
        .and_then(serde_json::Value::as_str)
    {
        Some("system") => Ok(None),
        Some("celsius") => Ok(Some(TemperatureDisplayUnit::Celsius)),
        Some("fahrenheit") => Ok(Some(TemperatureDisplayUnit::Fahrenheit)),
        _ => Err("unsupported temperature preference".into()),
    }
}

pub(crate) fn viewer_preferences(state: &Path) -> Result<Option<RegionalPreferences>, String> {
    let installation = read_installation(&state.join("installation.json"))?;
    let Some(selection) = installation.selected_host_configuration else {
        return Ok(None);
    };
    let checked = selection.checked()?;
    let mut regional = checked
        .configuration()
        .bases
        .iter()
        .filter(|base| base.kind == "locale/regional-preferences");
    let Some(base) = regional.next() else {
        return Ok(None);
    };
    if regional.next().is_some() {
        return Err("viewer requires one regional Base".into());
    }
    require_target(&checked, running_target_id()?)?;
    if !checked
        .resolved_bases()
        .iter()
        .any(|(kind, implementation)| kind == &base.kind && implementation == IMPLEMENTATION)
    {
        return Err("regional Base implementation differs from checked Host".into());
    }
    let selected = parameters(
        base.initialization
            .as_ref()
            .ok_or("regional Base has no checked initialization")?,
    )?;
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        Ok(Some(RegionalPreferences::observe(selected)))
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        let _ = selected;
        Err("regional preference Base requires GNU/Linux".into())
    }
}

#[cfg(all(test, target_os = "linux", target_env = "gnu", target_arch = "x86_64"))]
mod tests {
    use super::*;
    #[test]
    fn installed_regional_selection_reaches_foreground_viewer() {
        let (manifest, state) = crate::durable_host::tests::fixture();
        crate::durable_host::install(&manifest, &state).unwrap();
        assert!(viewer_preferences(&state).unwrap().is_none());
        let source = include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit")
            .replace("  limits =", r#"  base = {kind: "locale/regional-preferences", implementation: "hosted/linux-regional-preferences@1", initialization: {temperature: "fahrenheit"}}
  limits ="#);
        let path = state.join("regional.host.conduit");
        std::fs::write(&path, source).unwrap();
        super::super::configure(&state, Some(&path), false).unwrap();
        let preference = viewer_preferences(&state).unwrap().unwrap();
        assert_eq!(
            preference.temperature,
            Some(TemperatureDisplayUnit::Fahrenheit)
        );
        std::fs::remove_dir_all(state.parent().unwrap()).unwrap();
    }
}
