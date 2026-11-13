//! Explicit checked Source closure for MCU firmware fabrication. This runs on
//! the building Host; no parser, planner or duplicated Source graph enters MCU RAM.
use crate::software;
#[path = "workset_emission.rs"]
mod emission;
#[cfg(test)]
#[path = "workset_exact_work_tests.rs"]
mod exact_work_tests;
#[path = "workset_inputs.rs"]
mod inputs;
use conduit_plot as p;
use sha2::{Digest, Sha256};
use std::{fmt::Write, path::Path};
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
    pub(crate) installed_values: crate::installed_values::InstalledValueBindings,
    pub(crate) calibrated: bool,
    pub(crate) checked_source: p::CheckedSyntaxDocument,
    pub sha256: [u8; 32],
    pub entries: Vec<Entry>,
}
/// Absence is useful for equipment-only fabrication. The consuming selected
/// execution profile decides whether it requires a Source closure.
pub fn from_environment(mask: Option<MaskBounds>) -> Result<Option<Workset>, String> {
    inputs::from_environment(mask)
}
/// Internal finite Make subprocess receipt for existing typed-selection inputs.
/// Original Source declarations/imports remain authoritative; this is not a
/// human configuration language or an execution grant.
pub struct CapturedTypedSelections<'a> {
    pub bytes: &'a [u8],
    pub sha256: &'a str,
}
pub fn compile_captured_specializations(
    bytes: &[u8],
    expected_sha256: &str,
    entries: &[String],
    mask: Option<MaskBounds>,
    capture: CapturedTypedSelections<'_>,
) -> Result<Workset, String> {
    inputs::compile_captured(bytes, expected_sha256, entries, mask, capture)
}

pub fn compile(
    bytes: &[u8],
    expected_sha256: &str,
    entries: &[String],
    mask: Option<MaskBounds>,
) -> Result<Workset, String> {
    compile_specialized(bytes, expected_sha256, entries, mask, &[])
}
/// Explicit build-host specializations are checked against named Source Types.
/// They confer no runtime budget or initialized availability.
pub fn compile_specialized(
    bytes: &[u8],
    expected_sha256: &str,
    entries: &[String],
    mask: Option<MaskBounds>,
    specializations: &[crate::composite::Specialization<'_>],
) -> Result<Workset, String> {
    compile_specialized_with_installed(
        bytes,
        expected_sha256,
        entries,
        mask,
        specializations,
        &[],
        |_| Ok(Default::default()),
    )
}
/// Bind immutable equipment facts only after checking the original Source.
pub fn compile_specialized_with_installed(
    bytes: &[u8],
    expected_sha256: &str,
    entries: &[String],
    mask: Option<MaskBounds>,
    specializations: &[crate::composite::Specialization<'_>],
    installed_selections: &[crate::composite::ValueReference<'_>],
    prepare: impl FnOnce(
        &p::CheckedSyntaxDocument,
    ) -> Result<crate::installed_values::InstalledValueBindings, String>,
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
    conduit_semantic_catalog::install_resource_record_service_catalog(&mut startup, &mut profiles)?;
    crate::protocol_catalog::install(&mut startup, &mut profiles)?;
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
    crate::composite::install(&syntax, specializations, &mut startup, &mut profiles)?;
    crate::installed_values::install(&syntax, installed_selections, &mut startup, &mut profiles)?;
    let checked = p::check_syntax_document(&syntax, &startup).map_err(debug)?;
    let installed_values = prepare(&checked)?;
    installed_values.validate_source(&checked)?;
    // Install the ordinary checked selector definitions before expansion; the
    // Source compiler owns their meaning, not a target-side selector shortcut.
    for plot in &checked.plots {
        for cord in &plot.cords {
            for stage in &cord.stages {
                if let p::CheckedCordStage::StructuredSelector { selector, .. } = stage {
                    let definition = p::structured_selector_definition(
                        selector,
                        conduit_core::PortTemporal::Flow { closes: true },
                    );
                    // A checked closure can use the same selector at several
                    // sites. Reuse only its identical semantic definition;
                    // a conflicting definition still follows catalog refusal.
                    if profiles.get(&definition.kind_id) != Some(&definition) {
                        profiles.insert(definition).map_err(debug)?;
                    }
                }
            }
        }
    }

    // The broad checked-image index ceiling is only a host-tooling artifact
    // limit. Independent board calibration must still pass before emit_to.
    let image = conduit_embedded_build::EmbeddedImageBounds::HOST_TOOLING;
    let names = entries.iter().map(String::as_str).collect::<Vec<_>>();
    let prepared = conduit_embedded_build::prepare_checked_workset(
        &checked,
        &profiles,
        &names,
        conduit_embedded_build::CheckedWorksetBounds {
            maximum_entries: 8,
            maximum_nodes_per_entry: image.maximum_nodes,
            maximum_cords_per_entry: image.maximum_cords,
        },
    )?;
    let mut material = Vec::with_capacity(prepared.len());
    for (name, plot) in entries.iter().zip(prepared) {
        let expanded = &plot.expanded;
        let nodes = (0..expanded.gears.len())
            .map(|n| u16::try_from(n).map_err(|_| "node bound".to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut rust = software::emit_rust_for_layout_with_installed(
            expanded,
            Some(&checked),
            &nodes,
            24 * 1024,
            1024 * 1024,
            &installed_values,
        )?;
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
        installed_values,
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
    /// Every selected Gear receives distinct initialized Back custody, including
    /// repeated programs. Immutable program identity remains independent.
    pub fn prepare_selected_residents(
        &self,
        bounds: conduit_embedded_build::ResidentExpressionBounds,
    ) -> Result<Vec<conduit_embedded_build::PreparedResidentExpression>, String> {
        self.prepare_selected_residents_with_storage(
            bounds,
            conduit_embedded_build::ResidentExecutionStorage::Owned,
        )
    }
    /// Physical storage is selected before artifact sealing and calibration.
    /// The returned inventory still grants no execution or workspace capacity.
    pub fn prepare_selected_residents_with_storage(
        &self,
        bounds: conduit_embedded_build::ResidentExpressionBounds,
        storage: conduit_embedded_build::ResidentExecutionStorage,
    ) -> Result<Vec<conduit_embedded_build::PreparedResidentExpression>, String> {
        self.prepare_selected_residents_with_work_pool(bounds, storage, None)
    }
    /// Select each owner's exact compiler-reported requirement, then refuse if
    /// their checked aggregate exceeds the original shared private-work pool.
    /// Distinct initialized owners, including repeated programs, count once each.
    pub fn prepare_selected_residents_with_exact_work(
        &self,
        bounds: conduit_embedded_build::ResidentExpressionBounds,
        storage: conduit_embedded_build::ResidentExecutionStorage,
        capacity_units: u32,
    ) -> Result<Vec<conduit_embedded_build::PreparedResidentExpression>, String> {
        self.prepare_selected_residents_with_work_pool(bounds, storage, Some(capacity_units))
    }
    fn prepare_selected_residents_with_work_pool(
        &self,
        bounds: conduit_embedded_build::ResidentExpressionBounds,
        storage: conduit_embedded_build::ResidentExecutionStorage,
        capacity_units: Option<u32>,
    ) -> Result<Vec<conduit_embedded_build::PreparedResidentExpression>, String> {
        let mut selected = Vec::new();
        let mut required_work = 0u32;
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
                let program = p::PortableExpressionProgram::from_canonical_hex(text)
                    .map_err(|e| format!("selected resident program: {e:?}"))?;
                let instance = format!(
                    "{}:{}:{}",
                    self.checked_source.source_document_id.as_str(),
                    entry.name,
                    gear.gear_id.as_str()
                );
                let prepare = if capacity_units.is_some() {
                    conduit_embedded_build::PreparedResidentExpression::prepare_instance_with_exact_work
                } else {
                    conduit_embedded_build::PreparedResidentExpression::prepare_instance_with_storage
                };
                let image = prepare(
                    &self.checked_source,
                    &program,
                    temporal,
                    bounds,
                    Some(&instance),
                    storage,
                )
                .map_err(|e| {
                    format!(
                        "selected resident preparation for {}: {e:?}",
                        gear.gear_id.as_str()
                    )
                })?;
                if image.kind_id() != &gear.kind_id
                    || image.offer_description().semantic_contract != gear.semantic_contract
                {
                    return Err(format!(
                        "selected resident contract differs from checked gear {}",
                        gear.gear_id.as_str()
                    ));
                }
                if let Some(capacity) = capacity_units {
                    required_work = required_work
                        .checked_add(u32::from(image.computation_work()))
                        .ok_or("selected resident private-work sum overflow")?;
                    if required_work > capacity {
                        return Err(format!("selected resident private work requires {required_work} units; original pool has {capacity}"));
                    }
                }
                selected.push(image);
            }
        }
        Ok(selected)
    }
    /// Existing explicit DHT/Mask/TM execution adapter. Generic equipment uses
    /// emit_neutral_to so no role is inferred from unrelated native Kinds.
    pub fn emit_to(&self, out: &Path) -> Result<(), String> {
        emission::emit(self, out, true)
    }
    /// Emit exact checked entry definitions/layouts without execution role aliases.
    /// Structural calibration is mandatory; this does not install board execution.
    pub fn emit_neutral_to(&self, out: &Path) -> Result<(), String> {
        emission::emit(self, out, false)
    }
}
pub(crate) fn install(
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
