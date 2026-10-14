//! Read-only machine entrance to the installed owner's current semantic Face.

use std::path::Path;

use serde::Serialize;

const SCHEMA: &str = "conduit.body/local-face-snapshot@1";
// This envelope includes the same complete Host advertisement admitted by the
// local control response. A selected speech provider can exceed the smaller
// remote frame budget even though its Face is well within the Face Fore bound.
// Reserve finite space for the schema and decimal revision added on export.
const MAX_SNAPSHOT_METADATA_BYTES: usize = 256;
const MAX_JSON_BYTES: usize =
    crate::durable_host_control::MAXIMUM_LOCAL_CONTROL_FRAME_BYTES + MAX_SNAPSHOT_METADATA_BYTES;

#[derive(Serialize)]
struct Snapshot<'a> {
    schema: &'static str,
    face: &'a conduit_face::Face,
    face_revision_decimal: String,
    advertisement: &'a conduit_core::HostAdvertisement,
}

pub(crate) fn run(state_dir: &Path, json: bool) -> Result<(), String> {
    if !json {
        return Err("body face requires --json".into());
    }
    let (face, advertisement) = crate::durable_host_control::local_face_snapshot(state_dir)?;
    face.validate()
        .map_err(|error| format!("installed owner returned an invalid Face: {error:?}"))?;
    let bytes = serde_json::to_vec(&Snapshot {
        schema: SCHEMA,
        face: &face,
        face_revision_decimal: face.revision.to_string(),
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
