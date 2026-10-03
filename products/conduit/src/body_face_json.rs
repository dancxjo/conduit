//! Read-only machine entrance to the installed owner's current semantic Face.

use std::path::Path;

use serde::Serialize;

const SCHEMA: &str = "conduit.body/local-face-snapshot@1";
const MAX_JSON_BYTES: usize = 512 * 1024;

#[derive(Serialize)]
struct Snapshot<'a> {
    schema: &'static str,
    presentation: &'a conduit_presentation::Presentation,
    advertisement: &'a conduit_core::HostAdvertisement,
}

pub(crate) fn run(state_dir: &Path, json: bool) -> Result<(), String> {
    if !json {
        return Err("body face requires --json".into());
    }
    let (presentation, advertisement) =
        crate::durable_host_control::local_face_snapshot(state_dir)?;
    presentation
        .validate()
        .map_err(|error| format!("installed owner returned an invalid Face: {error:?}"))?;
    let bytes = serde_json::to_vec(&Snapshot {
        schema: SCHEMA,
        presentation: &presentation,
        advertisement: &advertisement,
    })
    .map_err(|error| format!("encode current Body Face: {error}"))?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err("current Body Face exceeds the machine-readable bound".into());
    }
    let value =
        String::from_utf8(bytes).map_err(|error| format!("encode current Body Face: {error}"))?;
    println!("{value}");
    Ok(())
}
