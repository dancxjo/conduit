//! Borrowed views of the canonical member-return documents for finite Hosts.
//!
//! These are transport representations, not admission or persistence authority.
//! They use the same fields and signing transcript as `PartReturnChallenge` and
//! `PartReturnProof`; no MCU-specific membership protocol is introduced.

use conduit_core::OfferGeneration;
use serde::{Deserialize, Serialize};

use crate::{ambient_admission_transcript, MAX_LIFECYCLE_ID_BYTES};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BorrowedPartReturnChallenge<'a> {
    pub admission_id: &'a str,
    pub body_id: &'a str,
    pub part_id: &'a str,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub offer_generation: u64,
    pub nonce: [u8; 32],
    pub issued_at_millis: u64,
    pub expires_at_millis: u64,
}

impl BorrowedPartReturnChallenge<'_> {
    pub fn signing_transcript(&self) -> [u8; 32] {
        ambient_admission_transcript(
            "part-return-v1",
            self.body_id,
            self.part_id,
            self.host_id,
            self.boot_id,
            OfferGeneration(self.offer_generation),
            &self.nonce,
            self.expires_at_millis,
        )
    }

    /// Check finite shape and exact retained membership/current incarnation.
    /// Owner time and nonce freshness remain the admitting owner's checks.
    pub fn matches_member(
        &self,
        body_id: &str,
        part_id: &str,
        host_id: &str,
        boot_id: &str,
        offer_generation: u64,
    ) -> bool {
        [
            self.admission_id,
            self.body_id,
            self.part_id,
            self.host_id,
            self.boot_id,
        ]
        .into_iter()
        .all(|id| !id.is_empty() && id.len() <= MAX_LIFECYCLE_ID_BYTES)
            && self.body_id == body_id
            && self.part_id == part_id
            && self.host_id == host_id
            && self.boot_id == boot_id
            && self.offer_generation == offer_generation
            && self.nonce != [0; 32]
            && self.expires_at_millis > self.issued_at_millis
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct BorrowedPartReturnProof<'a> {
    pub admission_id: &'a str,
    pub body_id: &'a str,
    pub part_id: &'a str,
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub nonce: [u8; 32],
    pub signature: &'a [u8],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdmissionId, BodyId, PartId, PartReturnChallenge};
    use conduit_core::{BootId, HostId};

    #[test]
    fn borrowed_return_preserves_canonical_wire_and_transcript() {
        let canonical = PartReturnChallenge {
            admission_id: AdmissionId::bound("admission/1".into()),
            body_id: BodyId::bound("body/1".into()),
            part_id: PartId::bound("part/1".into()),
            host_id: HostId::from("host/1"),
            boot_id: BootId::from("boot/2"),
            offer_generation: OfferGeneration(7),
            nonce: [3; 32],
            issued_at_millis: 10,
            expires_at_millis: 20,
        };
        let encoded = serde_json::to_vec(&canonical).unwrap();
        let borrowed: BorrowedPartReturnChallenge<'_> = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(
            borrowed.signing_transcript(),
            canonical.signing_transcript()
        );
        assert_eq!(serde_json::to_vec(&borrowed).unwrap(), encoded);
        assert!(borrowed.matches_member("body/1", "part/1", "host/1", "boot/2", 7));
        assert!(!borrowed.matches_member("body/other", "part/1", "host/1", "boot/2", 7));
        assert!(!borrowed.matches_member("body/1", "part/other", "host/1", "boot/2", 7));
        assert!(!borrowed.matches_member("body/1", "part/1", "host/1", "boot/old", 7));
        assert!(!borrowed.matches_member("body/1", "part/1", "host/1", "boot/2", 8));
    }
}

/// Fixed-size authentication metadata sent alongside exact raw receipt bytes.
/// The owner fields must match a peer selected independently by the Seed.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BorrowedAdmissionReceiptAttestation<'a> {
    pub schema: &'a str,
    #[serde(borrow)]
    pub owner: BorrowedFederationPeer<'a>,
    pub member_verifying_key: [u8; 32],
    #[serde(with = "signature_wire")]
    pub signature: [u8; 64],
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BorrowedFederationPeer<'a> {
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub offer_generation: u64,
    pub verifying_key: [u8; 32],
}

impl BorrowedAdmissionReceiptAttestation<'_> {
    pub fn verify(
        &self,
        receipt_json: &[u8],
        expected_owner: &crate::FederationPeer,
        expected_member_key: &[u8; 32],
    ) -> Result<(), crate::AdmissionReceiptAttestationRefusal> {
        let mut verifier = self.begin_verification(
            crate::AdmissionReceiptStreamBounds {
                receipt_bytes: receipt_json.len(),
                maximum_chunks: 1,
            },
            expected_owner,
            expected_member_key,
        )?;
        verifier.push(receipt_json)?;
        verifier.finish()
    }

    /// Pin metadata before accepting a bounded sequence of receipt bytes. This
    /// authenticates transport evidence only; it does not decode/admit a member.
    pub fn begin_verification<'a>(
        &'a self,
        bounds: crate::AdmissionReceiptStreamBounds,
        expected_owner: &'a crate::FederationPeer,
        expected_member_key: &[u8; 32],
    ) -> Result<crate::AdmissionReceiptVerifier<'a>, crate::AdmissionReceiptAttestationRefusal>
    {
        use crate::AdmissionReceiptAttestationRefusal as Refusal;
        if self.schema != crate::SIGNED_ADMISSION_RECEIPT_SCHEMA
            || self.owner.host_id != expected_owner.host_id.as_str()
            || self.owner.boot_id != expected_owner.boot_id.as_str()
            || self.owner.offer_generation != expected_owner.offer_generation.0
            || self.owner.verifying_key != expected_owner.verifying_key
        {
            return Err(Refusal::Owner);
        }
        if &self.member_verifying_key != expected_member_key {
            return Err(Refusal::MemberKey);
        }
        let transcript = crate::AdmissionReceiptTranscript::new(
            bounds,
            expected_owner.host_id.as_str(),
            expected_owner.boot_id.as_str(),
            expected_owner.offer_generation.0,
            &expected_owner.verifying_key,
            expected_member_key,
        )?;
        Ok(crate::AdmissionReceiptVerifier::new(
            transcript,
            expected_owner,
            &self.signature,
        ))
    }
}

mod signature_wire {
    use core::fmt;
    use serde::{
        de::{Error, SeqAccess, Visitor},
        Deserializer, Serialize, Serializer,
    };

    pub fn serialize<S: Serializer>(value: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error> {
        value.as_slice().serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 64], D::Error> {
        struct SignatureVisitor;
        impl<'de> Visitor<'de> for SignatureVisitor {
            type Value = [u8; 64];
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("exactly 64 signature bytes")
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut bytes = [0; 64];
                for (index, byte) in bytes.iter_mut().enumerate() {
                    *byte = sequence
                        .next_element()?
                        .ok_or_else(|| A::Error::invalid_length(index, &self))?;
                }
                if sequence.next_element::<u8>()?.is_some() {
                    return Err(A::Error::invalid_length(65, &self));
                }
                Ok(bytes)
            }
        }
        deserializer.deserialize_seq(SignatureVisitor)
    }
}
