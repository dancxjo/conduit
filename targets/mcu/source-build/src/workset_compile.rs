//! Build-Host adoption of the original Source and exact selected software.
use super::{
    debug, emit_mask_routing_images, install, Entry, MaskBounds, Workset, MAXIMUM_SOURCE_BYTES,
};
use crate::software;
use conduit_plot as p;
use sha2::{Digest, Sha256};
use std::fmt::Write;

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
