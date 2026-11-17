//! Standing semantic profiles retain finite storage without a cumulative quota.
use crate::AmbientDisplayAspect;
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};

pub const AMBIENT_RESIDENT_OBSERVE_KIND: &str = "ambient/resident-observation";
pub const AMBIENT_RESIDENT_OBSERVE_REVISION: &str = "ambient/resident-observation@1";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientUpdatePolicy {
    Finite(u32),
    Resident,
}
impl AmbientUpdatePolicy {
    pub fn exhausted(self, updates: u32) -> bool {
        matches!(self,Self::Finite(maximum) if updates>=maximum)
    }
}
pub fn ambient_resident_aspect_name(aspect: AmbientDisplayAspect) -> Option<&'static str> {
    match aspect {
        AmbientDisplayAspect::Wallpaper => Some("face/resident-ambient-wallpaper"),
        AmbientDisplayAspect::ClockWeather => Some("face/resident-ambient-clock-weather"),
        _ => None,
    }
}
pub fn ambient_resident_contributor_kind(aspect: AmbientDisplayAspect) -> Result<Kind, String> {
    let name = ambient_resident_aspect_name(aspect).ok_or("resident full Home is not offered")?;
    let mut kind = crate::ambient_aspect_contributor_kind(aspect)?;
    kind.kind_id = kind_id(name);
    kind.kind_contract_revision = format!("{name}@1").into();
    kind.startup_parameters
        .retain(|p| p.name != "maximum-updates");
    kind.configuration.retain(|p| p.key != "maximum-updates");
    kind.inputs[0].temporal = PortTemporal::Flow { closes: false };
    kind.validate()
        .map_err(|e| format!("resident contribution Kind: {e:?}"))?;
    Ok(kind)
}
pub fn ambient_resident_observe_kind() -> Result<Kind, String> {
    let mut kind = crate::ambient_observe_kind()?;
    kind.kind_id = kind_id(AMBIENT_RESIDENT_OBSERVE_KIND);
    kind.kind_contract_revision = AMBIENT_RESIDENT_OBSERVE_REVISION.into();
    kind.startup_parameters
        .retain(|p| p.name != "maximum-updates");
    kind.configuration.retain(|p| p.key != "maximum-updates");
    kind.inputs[0].temporal = PortTemporal::Flow { closes: false };
    kind.outputs[0].temporal = PortTemporal::Flow { closes: false };
    kind.validate()
        .map_err(|e| format!("resident observation Kind: {e:?}"))?;
    Ok(kind)
}
pub(crate) fn install_resident_contributors(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for aspect in [
        AmbientDisplayAspect::Wallpaper,
        AmbientDisplayAspect::ClockWeather,
    ] {
        let kind = ambient_resident_contributor_kind(aspect)?;
        startup.insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "label".into(),
                value_type: "Text".into(),
                default: None,
            }],
        })?;
        profile
            .insert_kind(kind)
            .map_err(|e| format!("resident contribution catalog: {e:?}"))?;
    }
    Ok(())
}
pub(crate) fn install_resident_observe(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: AMBIENT_RESIDENT_OBSERVE_KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "timezone".into(),
            value_type: "Text".into(),
            default: None,
        }],
    })?;
    profile
        .insert_kind(ambient_resident_observe_kind()?)
        .map_err(|e| format!("resident observation catalog: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resident_profiles_have_open_flows_and_no_cumulative_update_promise() {
        let observer = ambient_resident_observe_kind().unwrap();
        for ports in [&observer.inputs, &observer.outputs] {
            assert!(
                ports
                    .iter()
                    .all(|p| p.temporal == PortTemporal::Flow { closes: false })
            );
        }
        assert!(
            observer
                .configuration
                .iter()
                .all(|c| c.key != "maximum-updates")
        );
        for aspect in [
            AmbientDisplayAspect::Wallpaper,
            AmbientDisplayAspect::ClockWeather,
        ] {
            let kind = ambient_resident_contributor_kind(aspect).unwrap();
            assert_eq!(
                kind.inputs[0].temporal,
                PortTemporal::Flow { closes: false }
            );
            assert_eq!(kind.configuration.len(), 1);
            assert_eq!(kind.configuration[0].key, "label");
            assert!(
                kind.startup_parameters
                    .iter()
                    .all(|p| p.name != "maximum-updates")
            );
            assert_ne!(
                kind.kind_id,
                crate::ambient_aspect_contributor_kind(aspect)
                    .unwrap()
                    .kind_id
            );
        }
        assert!(
            crate::ambient_observe_kind()
                .unwrap()
                .configuration
                .iter()
                .any(|c| c.key == "maximum-updates")
        );
    }
}
