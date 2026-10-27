//! Validate actual guest forwarding, renderer and complete observation custody.
use super::*;
use conduit_face::{render_linear_face, Face};
use sha2::{Digest, Sha256};

pub(super) fn receipt(serial: &str) -> Result<Value, ConduitosError> {
    let records = rows(serial, "CONDUIT_NATIVE_FACE ")?;
    if records.is_empty() {
        return Err(refusal("collection Face observations absent"));
    }
    let mut measurements = Vec::new();
    for record in records {
        let face: Face =
            serde_json::from_value(record["face"].clone()).map_err(|e| refusal(e.to_string()))?;
        face.validate().map_err(|e| refusal(e.to_string()))?;
        let encoded = serde_json::to_vec(&face).map_err(|e| refusal(e.to_string()))?;
        let linear = render_linear_face(&face).map_err(|e| refusal(e.to_string()))?;
        let producer = &record["storage_receipts"]["producer"];
        let renderer = &record["storage_receipts"]["renderer"];
        let show = &record["show"];
        let digest = serde_json::to_value(<[u8; 32]>::from(Sha256::digest(&encoded)))
            .map_err(|e| refusal(e.to_string()))?;
        let collection = conduitos::native_face_snapshot::COLLECTION_SNAPSHOT_BYTES;
        let envelope = serde_json::to_vec(&record)
            .map_err(|e| refusal(e.to_string()))?
            .len();
        if encoded.len() > collection
            || envelope > collection + 32 * 1024
            || producer["encoded_bytes"] != encoded.len()
            || renderer["renderer_input_bytes"] != encoded.len()
            || !renderer["renderer_show_value_bytes"]
                .as_u64()
                .is_some_and(|n| n > 0 && n <= collection as u64)
            || producer["value_sha256"] != digest
            || [producer, renderer].iter().any(|r| {
                r["face_id"] != face.identity.as_str()
                    || r["face_revision"] != face.revision
                    || r["collection_storage"]["maximum_value_bytes"] != collection
            })
            || producer["producer_plan_id"] == renderer["mask_plan_id"]
            || renderer["mask_plan_id"] != show["mask_plan_id"]
            || renderer["active_play_id"] != show["mask_active_play_id"]
            || show["face_id"] != face.identity.as_str()
            || show["face_revision"] != face.revision
            || ["host_id", "boot_id", "offer_generation"]
                .iter()
                .any(|key| producer[*key] != show[*key])
        {
            return Err(refusal(
                "complete Collection Face/Plan/Show/storage custody mismatch",
            ));
        }
        measurements.push(json!({"face_id":face.identity,"revision":face.revision,
            "producer_bytes":encoded.len(),"observer_envelope_bytes":envelope,
            "linear_bytes":linear.encoded_bytes,"subjects":face.subjects.len(),
            "actions":face.actions.len(),"producer":producer,"renderer":renderer}));
    }
    Ok(
        json!({"schema":"conduit.conduitos/collection-face-admission@1",
        "proof_class":"freestanding-emulator","observations":measurements,
        "face_capacity_bytes":256*1024,"observer_capacity_bytes":288*1024,
        "remote_owner_route":"not exercised by local zero-Body Creche",
        "unsupported":"larger than declared capacity; held historical Source is separate from current Source"}),
    )
}
