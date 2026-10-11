//! Pin actual preparation provenance before the resident artifact is consumed.
use super::{PreparedProtocolArtifact, ProtocolBodyRefusal};
use conduit_body::{BodyPlan, BodyPlayIdentity};
use sha2::{Digest, Sha256};

pub(in crate::protocol_source) fn commitment(
    artifact: &PreparedProtocolArtifact,
    body: &BodyPlan,
    play: &BodyPlayIdentity,
) -> Result<Option<[u8; 32]>, ProtocolBodyRefusal> {
    let Some(package) = artifact.package_digest() else {
        return Ok(None);
    };
    if body.verify_seal().is_err()
        || !play.validate_for(body)
        || !body
            .plots
            .iter()
            .any(|resident| resident.plan == artifact.artifact().definition().internal_plan)
    {
        return Err(ProtocolBodyRefusal::UnsupportedWorkset);
    }
    let identity = artifact.artifact().identity();
    let mut hash = Sha256::new();
    frame(&mut hash, b"conduitos.storage-subordinate-preparation@1");
    frame(&mut hash, package);
    for value in [
        identity.source.as_str(),
        identity.checked.as_str(),
        identity.expanded.as_str(),
        identity.artifact.as_str(),
        artifact
            .artifact()
            .definition()
            .internal_plan
            .plan_id
            .as_str(),
        body.body_id.as_str(),
        body.wake_id.as_str(),
        body.plan_id.as_str(),
        play.active_play_id.as_str(),
    ] {
        frame(&mut hash, value.as_bytes());
    }
    // The verified immutable Plan seal commits every selected provider/resource
    // and exact Plot Back invocation path, not just the packaging artifact.
    hash.update(play.play_sequence.to_le_bytes());
    Ok(Some(hash.finalize().into()))
}

fn frame(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}
