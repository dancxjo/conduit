//! Pure Source-backed declared clock/control preparation; no Host or DSP authority.
use alloc::{format, string::String, vec};
use conduit_core::{CapabilityLimits, Kind, KindSemanticLaw, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CanonicalBackCatalog, CheckedSyntaxDocument,
    KindSignature, ProfileCatalog, StartupCatalog,
};

pub const ACOUSTIC_CONTROL_COMPONENTS: &[&str] = &[
    "audio/clock-binding-valid",
    "audio/clock-query-eligible",
    "audio/clock-project",
    "audio/control-query-time",
    "audio/control-final-time",
];

/// Allocating preparation retains the exact checked document and owner Type catalog.
/// It does not choose a Host, run Play or manufacture clock calibration.
pub struct AcousticControlSourcePackage {
    pub checked: CheckedSyntaxDocument,
    pub startup: StartupCatalog,
}

pub fn prepare_acoustic_control_source_package() -> Result<AcousticControlSourcePackage, String> {
    let mut startup = StartupCatalog::new();
    conduit_audio::authoring::install(&mut startup)?;
    let source = [
        conduit_audio::CLOCK_PROJECTION_SOURCE,
        conduit_audio::CONTROL_PROJECTION_SOURCE,
    ]
    .join("\n");
    let document = parse_syntax_document(&source);
    let checked = check_syntax_document(&document, &startup)
        .map_err(|error| format!("Audio control Source check: {error:?}"))?;
    Ok(AcousticControlSourcePackage { checked, startup })
}

fn definition(package: &AcousticControlSourcePackage, name: &str) -> Result<Kind, String> {
    let plot = package
        .checked
        .plots
        .iter()
        .find(|plot| plot.name == name)
        .ok_or_else(|| format!("missing Audio control Source entry: {name}"))?;
    let front = plot.checked_front();
    if !front.resource_ports().is_empty() || !front.startup_parameters().is_empty() {
        return Err(format!(
            "Audio control preparation must be closed and resource-free: {name}"
        ));
    }
    let kind = Kind {
        startup_parameters: vec![],
        shorthand: front.shorthand().map(|(a, b)| (a.clone(), b.clone())),
        kind_id: name.into(),
        kind_contract_revision: format!(
            "conduit.audio/control-source@1/{}",
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
    kind.validate()
        .map_err(|error| format!("Audio control Fore {name}: {error:?}"))?;
    if kind.checked_front() != front {
        return Err(format!("Audio control Source Fore changed: {name}"));
    }
    Ok(kind)
}

/// Atomically install exact owner Types and the five resource-free semantic Fores.
pub fn install_acoustic_control_catalogs(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    let package = prepare_acoustic_control_source_package()?;
    let mut staged_startup = startup.clone();
    let mut staged_profiles = profiles.clone();
    conduit_audio::authoring::install(&mut staged_startup)?;
    for name in ACOUSTIC_CONTROL_COMPONENTS {
        let kind = definition(&package, name)?;
        staged_startup.insert(KindSignature {
            kind: (*name).into(),
            startup_parameters: vec![],
        })?;
        staged_startup.insert_fore(name, kind.checked_front())?;
        staged_profiles
            .insert_kind(kind)
            .map_err(|error| format!("Audio control catalog: {error}"))?;
    }
    *startup = staged_startup;
    *profiles = staged_profiles;
    Ok(())
}

/// Atomically select exact owner checked Source Backs, with no Host capability.
pub fn install_acoustic_control_backs(
    profiles: &ProfileCatalog,
    backs: &mut CanonicalBackCatalog,
) -> Result<(), String> {
    let package = prepare_acoustic_control_source_package()?;
    let mut staged = backs.clone();
    for name in ACOUSTIC_CONTROL_COMPONENTS {
        let kind = definition(&package, name)?;
        if profiles.canonical_kind(&kind.kind_id) != Some(&kind) {
            return Err(format!(
                "Audio control contract differs from checked Source: {name}"
            ));
        }
        staged
            .insert(&kind, &package.checked, name)
            .map_err(|error| format!("Audio control Source Back {name}: {error:?}"))?;
    }
    *backs = staged;
    Ok(())
}
