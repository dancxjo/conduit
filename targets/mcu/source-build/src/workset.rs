//! Explicit checked Source closure for MCU firmware fabrication. This runs on
//! the building Host; no parser, planner or duplicated Source graph enters MCU RAM.
use crate::software;
use conduit_plot as p;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::{
    fmt::Write,
    path::{Path, PathBuf},
};
const MAXIMUM_SOURCE_BYTES: usize = 1024 * 1024;
/// Independently selected construction ceilings, not resource admission grants.
#[derive(Clone, Copy)]
pub struct MaskBounds {
    pub face_bytes: u32,
    pub show_bytes: u32,
}
pub struct Entry {
    /// Retained only on the building Host for ordinary structural calibration.
    pub plot: p::ExpandedAuthoringPlot,
    pub name: String,
    pub rust: String,
    pub nodes: usize,
    pub cords: usize,
}
pub struct Workset {
    pub(crate) calibrated: bool,
    checked_source: p::CheckedSyntaxDocument,
    pub sha256: [u8; 32],
    pub entries: Vec<Entry>,
}
/// Absence is useful for equipment-only fabrication. The consuming selected
/// execution profile decides whether it requires a Source closure.
pub fn from_environment(mask: Option<MaskBounds>) -> Result<Option<Workset>, String> {
    for name in [
        "CONDUIT_WORKSET_SOURCE",
        "CONDUIT_WORKSET_SOURCE_SHA256",
        "CONDUIT_WORKSET_ENTRIES",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    match (
        std::env::var_os("CONDUIT_WORKSET_SOURCE"),
        std::env::var("CONDUIT_WORKSET_SOURCE_SHA256"),
        std::env::var("CONDUIT_WORKSET_ENTRIES"),
    ) {
        (None, Err(std::env::VarError::NotPresent), Err(std::env::VarError::NotPresent)) => {
            Ok(None)
        }
        (Some(path), Ok(expected), Ok(entries)) => {
            let entries: Vec<String> =
                serde_json::from_str(&entries).map_err(|e| format!("workset entry list: {e}"))?;
            let path = PathBuf::from(path);
            println!("cargo:rerun-if-changed={}", path.display());
            let mut bytes = Vec::new();
            std::fs::File::open(&path)
                .map_err(|e| format!("captured workset Source: {e}"))?
                .take((MAXIMUM_SOURCE_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("captured workset Source: {e}"))?;
            compile(&bytes, &expected, &entries, mask).map(Some)
        }
        _ => Err(
            "workset fabrication requires Source, expected SHA256 and ordered entries together"
                .into(),
        ),
    }
}
pub fn compile(
    bytes: &[u8],
    expected_sha256: &str,
    entries: &[String],
    mask: Option<MaskBounds>,
) -> Result<Workset, String> {
    if bytes.is_empty()
        || bytes.len() > MAXIMUM_SOURCE_BYTES
        || entries.is_empty()
        || entries.len() > 8
        || entries
            .iter()
            .enumerate()
            .any(|(i, e)| e.is_empty() || e.len() > 128 || entries[..i].contains(e))
    {
        return Err("workset Source/entry bound".into());
    }
    let sha256: [u8; 32] = Sha256::digest(bytes).into();
    let actual = sha256.iter().fold(String::new(), |mut out, b| {
        write!(out, "{b:02x}").unwrap();
        out
    });
    if expected_sha256 != actual {
        return Err("captured workset Source SHA256 differs from fabrication input".into());
    }
    let source = core::str::from_utf8(bytes).map_err(|_| "workset Source is not UTF8")?;
    let (mut startup, mut profiles) = (p::StartupCatalog::new(), p::ProfileCatalog::new());
    conduit_pulse_capture::PulseReadContract::prepare()
        .map_err(debug)?
        .install(&mut startup, &mut profiles)?;
    conduit_dht11_observation::ObservationContract::prepare()
        .map_err(debug)?
        .install(&mut startup, &mut profiles)?;
    conduit_environment_runtime::recording::LogContract::prepare(
        &conduit_dht11_observation::ObservationContract::prepare().map_err(debug)?,
    )
    .map_err(debug)?
    .install(&mut startup, &mut profiles)?;
    for channel in conduit_environment_runtime::publication::PublicationChannel::ALL {
        conduit_environment_runtime::publication::PublicationContract::prepare(channel)
            .map_err(debug)?
            .install(&mut startup, &mut profiles)?;
    }
    conduit_semantic_catalog::install_resource_append_catalog(&mut startup, &mut profiles)?;
    conduit_time::install_tick_catalog(&mut startup, &mut profiles)?;
    install(
        conduit_time::tick_trigger_semantic_contract(),
        &mut startup,
        &mut profiles,
    )?;
    let tm = conduit_tm1637_wire::WriteContract::prepare().map_err(debug)?;
    install(tm.kind().clone(), &mut startup, &mut profiles)?;
    let absence =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::EMPTY_INFO_ID))
            .and_then(|ty| ty.profile())
            .map_err(debug)?;
    startup.insert_value_kind_alias("Tm1637Absence", absence.value_kind().clone())?;
    conduit_face::install_mask_plot_value_aliases(&mut startup)?;
    conduit_face::install_mask_mechanism_catalog(&mut startup, &mut profiles)?;
    let syntax = p::parse_syntax_document_with_glyph_notations(source, &startup);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("workset syntax: {:?}", syntax.diagnostics));
    }
    let checked = p::check_syntax_document(&syntax, &startup).map_err(debug)?;
    // Install the ordinary checked selector definitions before expansion; the
    // Source compiler owns their meaning, not a target-side selector shortcut.
    for plot in &checked.plots {
        for cord in &plot.cords {
            for stage in &cord.stages {
                if let p::CheckedCordStage::StructuredSelector { selector, .. } = stage {
                    profiles
                        .insert(p::structured_selector_definition(
                            selector,
                            conduit_core::PortTemporal::Flow { closes: true },
                        ))
                        .map_err(debug)?;
                }
            }
        }
    }

    let mut material = Vec::new();
    for name in entries {
        let plot =
            p::expand_canonical_plot_for_authoring(&checked, name, &profiles).map_err(debug)?;
        let expanded = &plot.expanded;
        let mut rust = software::emit_rust(expanded, 24 * 1024, 1024 * 1024)?;
        emit_mask_routing_images(expanded, &mut rust, mask)?;
        material.push(Entry {
            name: name.clone(),
            rust,
            nodes: expanded.gears.len(),
            cords: expanded.connections.len(),
            plot,
        });
    }
    Ok(Workset {
        calibrated: false,
        checked_source: checked,
        sha256,
        entries: material,
    })
}
impl Workset {
    /// Generate resident implementations from the same checked Source closure
    /// as the authored Plans. The document remains on the fabrication Host.
    pub fn prepare_residents(
        &self,
        entries: &[&str],
        bounds: conduit_embedded_build::ResidentExpressionBounds,
    ) -> Result<Vec<conduit_embedded_build::PreparedResidentExpression>, String> {
        conduit_embedded_build::prepare_source_resident_library(
            &self.checked_source,
            entries,
            bounds,
        )
    }

    /// Prepare the exact context-specialized expression programs selected by
    /// the expanded workset, rather than re-expanding separate named entries.
    /// Repeated instances require distinct physical Back custody; the current
    /// single-owner codebook profile refuses them instead of aliasing an owner.
    pub fn prepare_selected_residents(
        &self,
        bounds: conduit_embedded_build::ResidentExpressionBounds,
    ) -> Result<Vec<conduit_embedded_build::PreparedResidentExpression>, String> {
        let mut selected = Vec::new();
        let mut identities = Vec::new();
        for entry in &self.entries {
            for gear in &entry.plot.expanded.gears {
                let Some(configuration) = gear.configuration.iter().find(|c| c.key == "program")
                else {
                    continue;
                };
                let conduit_core::ConfigurationValue::Text(text) = &configuration.value else {
                    return Err("selected resident program is not canonical text".into());
                };
                let temporal = gear
                    .inputs
                    .first()
                    .ok_or("selected resident input absent")?
                    .temporal;
                if identities
                    .iter()
                    .any(|(old, mode)| old == text && *mode == temporal)
                {
                    return Err(
                        "repeated resident program requires distinct initialized instance custody"
                            .into(),
                    );
                }
                let program = p::PortableExpressionProgram::from_canonical_hex(text)
                    .map_err(|e| format!("selected resident program: {e:?}"))?;
                let image = conduit_embedded_build::PreparedResidentExpression::prepare(
                    &self.checked_source,
                    &program,
                    temporal,
                    bounds,
                )
                .map_err(|e| format!("selected resident preparation: {e:?}"))?;
                if image.kind_id() != &gear.kind_id
                    || image.offer_description().semantic_contract != gear.semantic_contract
                {
                    return Err(format!(
                        "selected resident contract differs from checked gear {}",
                        gear.gear_id.as_str()
                    ));
                }
                identities.push((text.clone(), temporal));
                selected.push(image);
            }
        }
        Ok(selected)
    }
    pub fn emit_to(&self, out: &Path) -> Result<(), String> {
        if !self.calibrated {
            return Err(
                "runtime factory emission requires independent checked structural calibration"
                    .into(),
            );
        }
        let mut index=format!("pub const SOURCE_SHA256:[u8;32]={:?};\npub const ENTRY_COUNT:usize={};\npub const ENTRY_NAMES:&[&str]=&{:?};\n",self.sha256,self.entries.len(),self.entries.iter().map(|e|e.name.as_str()).collect::<Vec<_>>());
        // Bind this finite execution profile to actual checked operations,
        // independent of user entry names or the signed workset's ordering.
        for (role, kind) in [
            ("sampling", "machine/pulse/read"),
            ("mask", "face/renderer"),
            ("output", "machine/tm1637/write"),
        ] {
            let selected = self
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| {
                    entry
                        .plot
                        .expanded
                        .gears
                        .iter()
                        .any(|g| g.kind_id.as_str() == kind)
                })
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            let [entry] = selected.as_slice() else {
                return Err(format!(
                    "execution profile requires exactly one checked {role} fragment"
                ));
            };
            writeln!(index, "pub use entry_{entry} as {role};").unwrap();
            writeln!(index, "pub use entry_{entry}_layout as {role}_layout;").unwrap();
        }
        for (i, entry) in self.entries.iter().enumerate() {
            let filename = format!("workset_{i}.rs");
            std::fs::write(out.join(&filename), &entry.rust).map_err(|e| e.to_string())?;
            writeln!(
                index,
                "pub mod entry_{i}{{include!(concat!(env!(\"OUT_DIR\"),\"/{filename}\"));}}"
            )
            .unwrap();
            writeln!(index,"pub mod entry_{i}_layout{{include!(concat!(env!(\"OUT_DIR\"),\"/workset_{i}_layout.rs\"));}}\n").unwrap();
        }
        std::fs::write(out.join("workset.rs"), index).map_err(|e| e.to_string())
    }
}
fn install(
    kind: conduit_core::Kind,
    startup: &mut p::StartupCatalog,
    profiles: &mut p::ProfileCatalog,
) -> Result<(), String> {
    if !kind.startup_parameters.is_empty() {
        return Err("unregistered parameterized workset contract".into());
    }
    startup.insert(p::KindSignature {
        kind: kind.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
    profiles.insert_kind(kind).map_err(debug)
}
/// These are descriptive definitions only. The runtime publishes them only
/// while its actual initialized ReadyMaskRouting owns the corresponding Backs.
pub(crate) fn emit_mask_routing_images(
    plot: &p::ExpandedCanonicalPlot,
    rust: &mut String,
    bounds: Option<MaskBounds>,
) -> Result<(), String> {
    use conduit_core as c;
    rust.push_str("pub static MASK_ROUTING_IMAGES:&[&[u8]]=&[\n");
    type Definition =
        fn(c::CapabilityId, c::ImplementationOffer, c::CapabilityLimits) -> c::CapabilityOffer;
    for gear in &plot.gears {
        let (identity, build): (&str, Definition) = match gear.kind_id.as_str() {
            "face/tee" => ("mcu.mask/tee@1", conduit_face::face_tee_offer),
            "face/interaction" => (
                "mcu.mask/interaction-close@1",
                conduit_face::face_interaction_close_offer,
            ),
            _ => continue,
        };
        let bounds =
            bounds.ok_or("Mask routing requires independently selected construction bounds")?;
        if bounds.face_bytes == 0
            || bounds.show_bytes == 0
            || bounds.face_bytes > 8192
            || bounds.show_bytes > 8192
        {
            return Err("unsupported Mask construction bounds".into());
        }
        let queue = if gear.kind_id.as_str() == "face/tee" {
            bounds.face_bytes
        } else {
            bounds.face_bytes.max(bounds.show_bytes)
        };
        let offer = build(
            identity.into(),
            c::ImplementationOffer {
                implementation_id: identity.into(),
                artifact_id: "conduit-mcu-host-runtime/mask-routing@1".into(),
                execution_profile_id: "conduit.mcu/prepared-mask-routing@1".into(),
            },
            c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: queue,
            },
        );
        let image = conduit_embedded_build::generate_capability_offer_image(&offer, 24 * 1024)
            .map_err(debug)?;
        writeln!(rust, "&{:?},", image).unwrap();
    }
    rust.push_str("];\n");
    Ok(())
}
fn debug(error: impl core::fmt::Debug) -> String {
    format!("checked MCU workset: {error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_sampling_mask_and_output_compile_from_one_checked_closure() {
        let source = [
            include_str!("../../../../plots/device-protocols/pulse-types.conduit"),
            include_str!("../../../../plots/device-protocols/dht11-capture.conduit"),
            include_str!("../../../../plots/device-protocols/dht11.conduit"),
            include_str!("../../../../plots/device-protocols/dht11-read.conduit"),
            include_str!("../../../../plots/device-protocols/dht11-observe.conduit"),
            include_str!("../../../../bodies/environment/profiles/sampling.conduit"),
            include_str!("../../../../bodies/environment/profiles/appliance.conduit"),
            include_str!("../../../../plots/device-protocols/tm1637.conduit"),
            include_str!("../../../../plots/device-protocols/tm1637-output.conduit"),
            include_str!("../../../../plots/seven-segment-mask/main.conduit"),
        ]
        .join("\n");
        let digest = format!("{:x}", Sha256::digest(source.as_bytes()));
        let entries =
            ["environmental-appliance", "seven-segment", "tm1637-output"].map(String::from);
        let bounds = Some(MaskBounds {
            face_bytes: 8192,
            show_bytes: 8192,
        });
        let workset = compile(source.as_bytes(), &digest, &entries, bounds).unwrap();
        assert_eq!(workset.entries.len(), 3);
        assert_eq!(workset.entries[1].nodes, 3);
        assert!(workset
            .entries
            .iter()
            .all(|e| e.rust.contains("CONFIGURATION")));
        assert!(compile(source.as_bytes(), &"0".repeat(64), &entries, bounds).is_err());
        assert!(compile(
            source.as_bytes(),
            &digest,
            &[entries[0].clone(), entries[0].clone()],
            bounds
        )
        .is_err());
        assert!(compile(
            &vec![0; MAXIMUM_SOURCE_BYTES + 1],
            &digest,
            &entries,
            bounds
        )
        .is_err());
    }
}
