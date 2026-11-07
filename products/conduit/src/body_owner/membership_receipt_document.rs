//! Typed input to the existing protected owner receipt-attestation operation.
use super::MemberPresentationBasis;
use conduit_body::{
    FederationPeer, MemberReturnAcknowledgement, PortableAdmissionReceipt,
    SignedPortableAdmissionReceipt,
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum MembershipReceiptDocument {
    Spawn(PortableAdmissionReceipt),
    Return(MemberReturnAcknowledgement),
    ImageReturn {
        receipt: conduit_body::MemberReturnImageAcknowledgement,
        images: Vec<Vec<u8>>,
    },
}
impl MembershipReceiptDocument {
    pub(super) fn presentation(&self) -> Result<MemberPresentationBasis, String> {
        match self {
            Self::Spawn(receipt) => MemberPresentationBasis::from_admission(receipt),
            Self::ImageReturn { receipt, images } => {
                receipt
                    .validate_for(&receipt.credential, &receipt.projection)
                    .map_err(|e| format!("image return acknowledgement: {e:?}"))?;
                let advertisement = conduit_std_host::pico_spawn::resolve_member_offer_images(
                    &receipt.projection,
                    images,
                )
                .map_err(|e| format!("image return resolution: {e:?}"))?;
                MemberPresentationBasis::new(receipt.credential.clone(), advertisement)
            }
            Self::Return(receipt) => {
                receipt
                    .validate_for(&receipt.credential, &receipt.advertisement)
                    .map_err(|e| format!("return acknowledgement: {e:?}"))?;
                MemberPresentationBasis::new(
                    receipt.credential.clone(),
                    receipt.advertisement.clone(),
                )
            }
        }
    }
    pub(super) fn attest(
        &self,
        owner: FederationPeer,
        member_key: [u8; 32],
        sign: impl FnOnce(&[u8; 32]) -> [u8; 64],
    ) -> Result<SignedPortableAdmissionReceipt, String> {
        match self {
            Self::Spawn(receipt) => {
                conduit_body::attest_portable_admission_receipt(receipt, owner, member_key, sign)
            }
            Self::Return(receipt) => {
                conduit_body::attest_member_return_acknowledgement(receipt, owner, member_key, sign)
            }
            Self::ImageReturn { receipt, .. } => {
                conduit_body::attest_member_return_image_acknowledgement(
                    receipt, owner, member_key, sign,
                )
            }
        }
        .map_err(|e| format!("membership receipt attestation: {e:?}"))
    }
}
