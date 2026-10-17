//! Reviewed Source-backed DSP interfaces, never an installed Host capability.
use alloc::{collections::BTreeSet, format, string::String, vec};
use conduit_core::*;
use conduit_plot::{
    CanonicalBackCatalog, CheckedCordStage, KindSignature, ProfileCatalog, StartupCatalog,
};
const COMPONENTS: &[&str] = &[
    "oscillator-prepare",
    "oscillator-step",
    "resonator-prepare",
    "resonator-step",
    "noise-step-candidate",
    "noise-step-value",
    "envelope",
    "envelope-value",
    "gain",
    "gain-value",
    "mix-two",
    "mix-two-value",
    "mix-four",
    "one-pole",
    "noise-gain-example",
    "experimental-q14-frame",
    "experimental-q14-control-frame",
    "experimental-q14-two-source",
    "experimental-independent-excitation",
    "shared-excitation-profile-start",
    "shared-excitation-music-start",
];
fn definition(package: &super::DspSourcePackage, leaf: &str) -> Result<(String, Kind), String> {
    let path = format!("audio/dsp/{leaf}");
    let syntax = package
        .exports
        .resolve(&path)
        .ok_or_else(|| format!("missing DSP export {path}"))?;
    if !syntax.front.type_parameters.is_empty()
        || !syntax.front.kind_parameters.is_empty()
        || !syntax.front.startup_parameters.is_empty()
    {
        return Err(format!("DSP public entry requires a closed Fore: {path}"));
    }
    let plot = package
        .checked
        .plots
        .iter()
        .find(|p| p.name == syntax.name.text)
        .ok_or_else(|| format!("missing checked DSP entry {path}"))?;
    let front = plot.checked_front();
    if !front.resource_ports().is_empty() {
        return Err(format!(
            "DSP Source Fore must not acquire resource authority: {path}"
        ));
    }
    let definition = Kind {
        startup_parameters: front.startup_parameters().to_vec(),
        shorthand: front.shorthand().map(|(a, b)| (a.clone(), b.clone())),
        kind_id: path.into(),
        kind_contract_revision: format!(
            "conduit.audio/dsp-source@1/{}",
            package.checked.source_document_id.as_str()
        )
        .into(),
        inputs: front.inputs().to_vec(),
        outputs: front.outputs().to_vec(),
        configuration: vec![],
        semantic_laws: vec![KindSemanticLaw::ValueContracts(
            front.value_contracts().to_vec(),
        )],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    };
    definition
        .validate()
        .map_err(|e| format!("DSP Fore {leaf}: {e:?}"))?;
    if definition.checked_front() != front {
        return Err(format!("DSP Source Fore changed: {leaf}"));
    }
    Ok((plot.name.clone(), definition))
}
/// Stage only public Types and semantic Fores; private pair profiles stay owned.
pub fn install_dsp_source_catalogs(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    let package = super::prepare_dsp_source_package()?;
    let mut staged_startup = startup.clone();
    let mut staged_profiles = profiles.clone();
    package
        .exports
        .install_shipped_types(&mut staged_startup)
        .map_err(|e| format!("DSP shipped Types: {e:?}"))?;
    for leaf in COMPONENTS {
        let (_, kind) = definition(&package, leaf)?;
        staged_startup.insert(KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        staged_startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        staged_profiles
            .insert_kind(kind)
            .map_err(|e| format!("DSP Source catalog: {e}"))?;
    }
    *startup = staged_startup;
    *profiles = staged_profiles;
    Ok(())
}
/// Select exact checked code with its owner-only primitive contracts and Types.
pub fn install_dsp_source_backs(
    profiles: &ProfileCatalog,
    backs: &mut CanonicalBackCatalog,
) -> Result<(), String> {
    let package = super::prepare_dsp_source_package()?;
    let mut owner = package.profiles.clone();
    let mut selectors = BTreeSet::new();
    for plot in &package.checked.plots {
        for stage in plot.cords.iter().flat_map(|c| &c.stages) {
            if let CheckedCordStage::StructuredSelector { selector, .. } = stage {
                let front = plot.checked_front();
                if !front
                    .inputs()
                    .iter()
                    .chain(front.outputs())
                    .all(|p| p.temporal == (PortTemporal::Flow { closes: true }))
                {
                    return Err(format!(
                        "DSP selector profile needs an explicit temporal declaration: {}",
                        plot.name
                    ));
                }
                let selector = conduit_plot::structured_selector_definition(
                    selector,
                    PortTemporal::Flow { closes: true },
                );
                if selectors.insert(selector.kind_id.clone()) {
                    owner
                        .insert(selector)
                        .map_err(|error| format!("DSP selector profile: {error}"))?;
                }
            }
        }
    }
    let mut staged = backs.clone();
    for leaf in COMPONENTS {
        let (plot, kind) = definition(&package, leaf)?;
        if profiles.canonical_kind(&kind.kind_id) != Some(&kind) {
            return Err(format!(
                "DSP Source contract differs from checked package: {leaf}"
            ));
        }
        staged
            .insert_with_catalog(&kind, &package.checked, &plot, &owner)
            .map_err(|e| format!("DSP Source Back {leaf}: {e:?}"))?;
    }
    *backs = staged;
    Ok(())
}
