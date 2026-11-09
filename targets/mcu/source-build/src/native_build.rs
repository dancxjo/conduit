//! Ordinary checked native calibration/emission shared by target build scripts.
//! This never replaces resolved-Base checks, initializes hardware or admits work.
use crate::{native_calibration as calibration, recording, workset};
use conduit_core::CapabilityOffer;
use conduit_host_make::CheckedHostConfiguration;
use std::{fs, path::Path};

pub fn checked_recording(
    checked: &CheckedHostConfiguration,
    implementation: &str,
    minimum_prefix_bytes: u32,
    durable_bytes: u32,
) -> Result<Option<recording::CheckedRecordingConstruction>, String> {
    if !checked
        .configuration()
        .bases
        .iter()
        .any(|base| base.kind == recording::BASE_KIND)
    {
        return Ok(None);
    }
    let construction =
        recording::CheckedRecordingConstruction::from_configuration(checked, implementation)?;
    if construction.prefix_bytes < minimum_prefix_bytes || construction.end_bytes()? > durable_bytes
    {
        return Err("recording exceeds physical DURABLE".into());
    }
    Ok(Some(construction))
}

/// Preserve original standard image ordering and optional append shape. Every
/// definition still passes the same finite ordinary image encoder.
pub fn emit_capability_images(
    out: &Path,
    named: &[(&str, &CapabilityOffer)],
    append: Option<&CapabilityOffer>,
    maximum_bytes: usize,
) -> Result<(), String> {
    let mut native = String::new();
    for (name, definition) in named {
        let bytes =
            conduit_embedded_build::generate_capability_offer_image(definition, maximum_bytes)
                .map_err(|e| format!("finite checked native image: {e:?}"))?;
        native.push_str(&format!("pub static {name}:&[u8]=&{bytes:?};\n"));
    }
    if let Some(offer) = append {
        let bytes = conduit_embedded_build::generate_capability_offer_image(offer, maximum_bytes)
            .map_err(|e| format!("finite checked append image: {e:?}"))?;
        native.push_str(&format!(
            "pub static APPEND_IMAGE:Option<&[u8]>=Some(&{bytes:?});\n"
        ));
    } else {
        native.push_str("pub static APPEND_IMAGE:Option<&[u8]>=None;\n");
    }
    fs::write(out.join("native_images.rs"), native).map_err(|e| e.to_string())
}

/// Compile through the existing independent native calibration, including its
/// exact checked resolved-Base correspondence, then emit the same artifacts.
pub fn calibrate_and_emit(
    workset: &mut workset::Workset,
    mask: Option<workset::MaskBounds>,
    out: &Path,
    input: calibration::Input<'_>,
) -> Result<(), String> {
    let assignment_bytes = input
        .checked
        .configuration()
        .preparation
        .as_ref()
        .ok_or("missing native preparation policy")?
        .maximum_assignment_bytes;
    fs::write(
        out.join("workset-policy.json"),
        input.bounds.policy_json(assignment_bytes).to_string(),
    )
    .map_err(|e| e.to_string())?;
    let calibrated = calibration::compile(workset, input)?;
    calibrated.apply_software_mapping(workset, mask, 24 * 1024, 1024 * 1024)?;
    calibrated.emit_continuation_identity(out, workset.sha256)?;
    calibrated.emit_to(out, 2 * 1024 * 1024)?;
    crate::source_composition::emit_to(out).map_err(|e| e.to_string())?;
    workset.emit_to(out)
}

/// Preserve the existing one-sampling-clock profile and renderer reservation;
/// Source periods cannot enlarge independently checked initialized timer bounds.
pub fn check_source_clocks(
    workset: &workset::Workset,
    clock: crate::clock::CheckedClockConstruction,
) -> Result<(), String> {
    let mut waits = 0usize;
    for entry in &workset.entries {
        let material = crate::software::compile(&entry.plot.expanded, 24 * 1024)?;
        for node in material.nodes {
            if let crate::software::SoftwareRole::Tick { period_ms, .. } = node.role {
                waits += 1;
                if period_ms > clock.maximum_duration_ms {
                    return Err("Source wait exceeds selected native clock".into());
                }
            }
        }
    }
    if waits != 1 {
        return Err("this finite native execution profile owns one sampling clock".into());
    }
    if waits + 1 > clock.slots {
        return Err("Source waits and renderer exceed actual initialized clock slots".into());
    }
    Ok(())
}
