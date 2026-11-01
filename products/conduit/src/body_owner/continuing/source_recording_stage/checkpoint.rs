//! Prepare one immutable prefix using the shared codec after worker quiescence.
//! The original staged observations remain owned until actual catalog commitment.
use super::*;
use conduit_core as c;
use conduit_environment_runtime::source_recording::{
    Refusal as CodecRefusal, SourceRecordingContext, SourceRecordingSnapshot,
};
use sha2::{Digest, Sha256};

pub(super) struct PreparedSourceChunk {
    pub(super) snapshot: SourceRecordingSnapshot,
    pub(super) reference: c::BoundedResourceRef,
}
impl SourceRecordingStage {
    /// This is preparation custody, never publication or a durable receipt.
    /// Fore sequences start at zero; recording sequences start at one. This
    /// explicit stable mapping does not reinterpret a device journal counter.
    pub(in super::super::super) fn prepare_checkpoint_input(&mut self) -> Result<(), String> {
        if self.prepared.is_some() || self.length == 0 {
            return Ok(());
        }
        let scope = self
            .scope()
            .ok_or("staged Source lacks actual Play scope")?;
        let s = scope.selection.as_ref();
        let context = SourceRecordingContext {
            body_id: self.body_id.as_str().into(),
            attachment_id: s.attachment_id.clone(),
            host_id: s.host_id.clone(),
            boot_id: s.boot_id.clone(),
            capability_id: s.capability_id.clone(),
            provider_instance_id: s.provider_instance_id.clone(),
            provider_generation: s.provider_generation,
            plan_id: s.plan_id.clone(),
            fragment_id: s.fragment_id.clone(),
            placement_id: s.placement_id.clone(),
            configuration_id: s.configuration_id.clone(),
            firmware_sha256: s.firmware_sha256,
            native_play: scope.native_play.clone(),
        };
        let mut snapshot = SourceRecordingSnapshot::prepare(context).map_err(codec_error)?;
        for observation in self.observations() {
            let sequence = observation
                .sequence()
                .checked_add(1)
                .ok_or("recording sequence exhausted")?;
            match snapshot.append(sequence, observation.canonical()) {
                Ok(_) => {}
                // The next finite chunk will retain the remainder. Never
                // advance or discard staged records from encoding alone.
                Err(CodecRefusal::Capacity) if !snapshot.is_empty() => break,
                Err(error) => return Err(codec_error(error)),
            }
        }
        let content = snapshot.content().map_err(codec_error)?;
        let digest: [u8; 32] = Sha256::digest(content).into();
        let reference = c::BoundedResourceRef {
            identity: c::ResourceSemanticIdentity::from_digest(digest),
            content_profile: "text/json-utf8@1".into(),
            access_class: "resource/snapshot@1".into(),
            extent: c::ResourceExtent {
                bytes: content.len() as u64,
                items: Some(1),
            },
            lifetime: c::ResourceLifetime {
                version: c::ResourceVersionIdentity::from_digest(digest),
                expires_at: None,
            },
        };
        reference
            .validate()
            .map_err(|e| format!("recording reference: {e:?}"))?;
        self.prepared = Some(PreparedSourceChunk {
            snapshot,
            reference,
        });
        Ok(())
    }
    pub(in super::super::super) fn prepared_input_metadata(
        &self,
    ) -> Result<Option<serde_json::Value>, String> {
        self.prepared
            .as_ref()
            .map(|prepared| {
                let encoded = prepared
                    .reference
                    .encode()
                    .map_err(|e| format!("recording reference: {e:?}"))?;
                let reference_hex: String = encoded.iter().map(|b| format!("{b:02x}")).collect();
                Ok(serde_json::json!({
                    "custody": "prepared-content-not-published-or-recorded",
                    "reference_hex": reference_hex,
                    "canonical_sha256": prepared.reference.identity.digest().iter().map(|b| format!("{b:02x}")).collect::<String>(),
                    "canonical_bytes": prepared.snapshot.content().map_err(codec_error)?.len(),
                    "source_scope": prepared.snapshot.context(),
                    "records": prepared.snapshot.len(),
                    "pending_records": self.len(),
                    "producer_retirement_authorized": false,
                }))
            })
            .transpose()
    }
}
fn codec_error(error: CodecRefusal) -> String {
    format!("Source checkpoint preparation: {error:?}")
}
