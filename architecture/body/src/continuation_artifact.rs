//! Bounded installed recipe metadata, not installed facts or execution authority.
//!
//! Root independently owns the media range, membership facts, accepted generation,
//! checked Make recipe pins and issuer policy. Decoding any bytes here proves none
//! of those. Signature verification and fresh complete admission remain separate.
use crate as b;
use serde::{Deserialize, Serialize};

pub const PREFIX_BYTES: usize = 8192;
/// The physical prefix remains8192 bytes; its final2048 bytes hold Root's
/// committed acceptance, not a larger artifact or recording grant.
pub const ACCEPTANCE_BYTES: usize = 2048;
pub const ARTIFACT_BYTES: usize = PREFIX_BYTES - ACCEPTANCE_BYTES;
const HEADER_BYTES: usize = 20;
const MAGIC: &[u8; 16] = b"CONDUIT_CONT@1\0\0";
pub const SCHEMA: &str = "conduit.host/installed-continuation@1";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlotPin<'a> {
    pub source_document: &'a str,
    pub checked_plot: &'a str,
    pub plan_id: &'a str,
    pub local: bool,
}
/// Descriptive full Wake and signed intent. Fixed slots impose a finite decode
/// bound; exactly `plot_count` leading slots must be occupied, the rest empty.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact<'a> {
    pub schema: &'a str,
    #[serde(borrow)]
    pub intent: b::ContinuationIntent<'a>,
    /// Detached signature bytes, split solely to use serde's existing fixed
    /// array support. Reassembled unchanged before existing signature checking.
    pub signature: [[u8; 32]; 2],
    pub wake_id: &'a str,
    pub wake_sequence: u64,
    pub workload_revision: u64,
    pub base_body_plan: &'a str,
    pub plot_count: u8,
    #[serde(borrow)]
    pub plots: [Option<PlotPin<'a>>; b::MAX_BODY_PLOTS],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Extent,
    Header,
    Encoding,
    Noncanonical,
    Bounds,
    Scope,
}
impl Artifact<'_> {
    pub fn signature_bytes(&self) -> [u8; 64] {
        let mut bytes = [0; 64];
        bytes[..32].copy_from_slice(&self.signature[0]);
        bytes[32..].copy_from_slice(&self.signature[1]);
        bytes
    }
    /// Validate descriptive scope using the existing Body transcript. This is
    /// deliberately not signature verification or construction of trusted facts.
    pub fn validate(&self) -> Result<(), Refusal> {
        let count = usize::from(self.plot_count);
        if self.schema != SCHEMA
            || self.intent.schema != b::CONTINUATION_INTENT_SCHEMA
            || !self.intent.ceilings.has_finite_bounds()
            || self.intent.source_digest == [0; 32]
            || self.intent.workset_digest == [0; 32]
            || count == 0
            || count > b::MAX_BODY_PLOTS
            || self.plots[..count].iter().any(Option::is_none)
            || self.plots[count..].iter().any(Option::is_some)
        {
            return Err(Refusal::Bounds);
        }
        for id in [
            self.intent.body_id,
            self.intent.part_id,
            self.intent.host_id,
            self.intent.credential_id,
            self.intent.issuer_host,
            self.intent.image_id,
            self.intent.configuration_id,
        ] {
            if id.is_empty() || id.len() > b::MAX_LIFECYCLE_ID_BYTES {
                return Err(Refusal::Bounds);
            }
        }
        self.with_scope(|scope| b::continuation_scope_digest(self.intent.body_id, scope))?
            .map_err(|_| Refusal::Scope)
            .and_then(|digest| {
                if digest == self.intent.scope_digest {
                    Ok(())
                } else {
                    Err(Refusal::Scope)
                }
            })
    }
    /// Borrow reconstructed full-Wake pins only during the callback. They do not
    /// authenticate their origin; callers must retain independent installed facts.
    pub fn with_scope<R>(
        &self,
        callback: impl FnOnce(&b::ContinuationFullScope<'_>) -> R,
    ) -> Result<R, Refusal> {
        let count = usize::from(self.plot_count);
        if count == 0 || count > b::MAX_BODY_PLOTS {
            return Err(Refusal::Bounds);
        }
        let mut pins = [b::ContinuationPlotPin {
            source_document: "",
            checked_plot: "",
            plan_id: "",
            local: false,
        }; b::MAX_BODY_PLOTS];
        for (out, pin) in pins[..count].iter_mut().zip(&self.plots[..count]) {
            let pin = pin.ok_or(Refusal::Bounds)?;
            *out = b::ContinuationPlotPin {
                source_document: pin.source_document,
                checked_plot: pin.checked_plot,
                plan_id: pin.plan_id,
                local: pin.local,
            };
        }
        Ok(callback(&b::ContinuationFullScope {
            wake_id: self.wake_id,
            wake_sequence: self.wake_sequence,
            workload_revision: self.workload_revision,
            base_body_plan: self.base_body_plan,
            plots: &pins[..count],
        }))
    }
}
/// Caller funds the full prefix before preparation. No allocation or media effect.
pub fn encode(artifact: &Artifact<'_>, output: &mut [u8; PREFIX_BYTES]) -> Result<(), Refusal> {
    artifact.validate()?;
    output.fill(0xff);
    let length = serde_json_core::to_slice(artifact, &mut output[HEADER_BYTES..ARTIFACT_BYTES])
        .map_err(|_| Refusal::Extent)?;
    output[..16].copy_from_slice(MAGIC);
    output[16..HEADER_BYTES].copy_from_slice(&(length as u32).to_le_bytes());
    Ok(())
}
/// Borrow IDs from an exact installed prefix. Scratch is caller-funded temporary
/// preparation workspace, not another retained prefix or new Root grant. Exact
/// canonical re-encoding rejects alternate JSON spellings, duplicate fields and
/// trailing data. Erased padding is part of this artifact's finite format.
pub fn decode<'a>(
    input: &'a [u8],
    scratch: &mut [u8; PREFIX_BYTES],
) -> Result<Artifact<'a>, Refusal> {
    decode_inner(input, scratch, false)
}
/// Decode only descriptive artifact bytes from an accepted physical prefix.
/// This does NOT verify the separate Root acceptance or authorize execution.
pub fn decode_installed<'a>(
    input: &'a [u8],
    scratch: &mut [u8; PREFIX_BYTES],
) -> Result<Artifact<'a>, Refusal> {
    decode_inner(input, scratch, true)
}
fn decode_inner<'a>(
    input: &'a [u8],
    scratch: &mut [u8; PREFIX_BYTES],
    accepted: bool,
) -> Result<Artifact<'a>, Refusal> {
    if input.len() != PREFIX_BYTES {
        return Err(Refusal::Extent);
    }
    if &input[..16] != MAGIC {
        return Err(Refusal::Header);
    }
    let length = usize::try_from(u32::from_le_bytes(
        input[16..HEADER_BYTES]
            .try_into()
            .map_err(|_| Refusal::Header)?,
    ))
    .map_err(|_| Refusal::Extent)?;
    if length == 0 || length > ARTIFACT_BYTES - HEADER_BYTES {
        return Err(Refusal::Extent);
    }
    let end = HEADER_BYTES + length;
    let padding_end = if accepted {
        ARTIFACT_BYTES
    } else {
        PREFIX_BYTES
    };
    if input[end..padding_end].iter().any(|byte| *byte != 0xff) {
        return Err(Refusal::Noncanonical);
    }
    let (artifact, used) = serde_json_core::from_slice::<Artifact<'a>>(&input[HEADER_BYTES..end])
        .map_err(|_| Refusal::Encoding)?;
    if used != length {
        return Err(Refusal::Noncanonical);
    }
    encode(&artifact, scratch)?;
    if scratch[..ARTIFACT_BYTES] != input[..ARTIFACT_BYTES]
        || (!accepted && scratch[ARTIFACT_BYTES..] != input[ARTIFACT_BYTES..])
    {
        return Err(Refusal::Noncanonical);
    }
    Ok(artifact)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_carriers_are_rejected_before_any_authority_check() {
        let mut scratch = [0; PREFIX_BYTES];
        assert!(matches!(decode(&[], &mut scratch), Err(Refusal::Extent)));
        let mut bytes = [0xff; PREFIX_BYTES];
        assert!(matches!(decode(&bytes, &mut scratch), Err(Refusal::Header)));
        bytes[..16].copy_from_slice(MAGIC);
        bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
        bytes[20] = b'{';
        assert!(matches!(
            decode(&bytes, &mut scratch),
            Err(Refusal::Encoding)
        ));
        bytes[PREFIX_BYTES - 1] = 0;
        assert!(matches!(
            decode(&bytes, &mut scratch),
            Err(Refusal::Noncanonical)
        ));
        bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(matches!(decode(&bytes, &mut scratch), Err(Refusal::Extent)));
    }
    #[test]
    fn installed_decode_keeps_acceptance_separate_from_descriptive_artifact() {
        let mut plots = [None; b::MAX_BODY_PLOTS];
        plots[0] = Some(PlotPin {
            source_document: "source",
            checked_plot: "checked",
            plan_id: "plan",
            local: true,
        });
        let wake_bytes = b::identity::bound_identity_bytes("wake", &["body"], 1);
        let wake = core::str::from_utf8(&wake_bytes).unwrap();
        let digest = b::body_plan_commitment_digest(
            "body",
            wake,
            1,
            &[b::BodyPlotSeal {
                source_document_id: "source",
                checked_plot_id: "checked",
                plan_id: "plan",
            }],
            &[],
            None,
        )
        .unwrap();
        let mut base = alloc::string::String::from("body-plan/sha256:");
        use core::fmt::Write;
        for byte in digest {
            write!(&mut base, "{byte:02x}").unwrap();
        }
        let mut artifact = Artifact {
            schema: SCHEMA,
            intent: b::ContinuationIntent {
                schema: b::CONTINUATION_INTENT_SCHEMA,
                body_id: "body",
                part_id: "part",
                host_id: "host",
                credential_id: "credential",
                membership_generation: 1,
                intent_generation: 1,
                issuer_host: "owner",
                issuer_key: [7; 32],
                source_digest: [1; 32],
                workset_digest: [2; 32],
                authority: b::ContinuationAuthority::LocalWorksetReplacement,
                scope_digest: [0; 32],
                image_id: "image",
                configuration_id: "configuration",
                ceilings: b::ContinuationCeilings {
                    assignment_bytes: 4096,
                    runtime_bytes: 4096,
                    durable_bytes: 8192,
                    operation_slots: 2,
                    active_worksets: 1,
                },
            },
            // Deliberately unauthenticated: decoding must not verify or repair it.
            signature: [[0; 32]; 2],
            wake_id: wake,
            wake_sequence: 1,
            workload_revision: 1,
            base_body_plan: &base,
            plot_count: 1,
            plots,
        };
        artifact.intent.scope_digest = artifact
            .with_scope(|scope| b::continuation_scope_digest(artifact.intent.body_id, scope))
            .unwrap()
            .unwrap();
        let mut bytes = [0; PREFIX_BYTES];
        let mut scratch = [0; PREFIX_BYTES];
        encode(&artifact, &mut bytes).unwrap();
        assert!(decode(&bytes, &mut scratch).is_ok());
        for value in [0, 0x5a] {
            bytes[ARTIFACT_BYTES..].fill(value);
            assert_eq!(
                decode(&bytes, &mut scratch).err(),
                Some(Refusal::Noncanonical)
            );
            let decoded = decode_installed(&bytes, &mut scratch).unwrap();
            assert_eq!(decoded.signature_bytes(), [0; 64]);
            assert_eq!(decoded.intent.scope_digest, artifact.intent.scope_digest);
        }
        // Installed decoding still enforces erased padding in the artifact range.
        bytes[ARTIFACT_BYTES - 1] = 0;
        assert_eq!(
            decode_installed(&bytes, &mut scratch).err(),
            Some(Refusal::Noncanonical)
        );
    }
}
