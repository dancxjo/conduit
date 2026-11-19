//! Optional preparation-side image receiver over the existing framed transport.
//! Observation, proof checking and receipt delivery are distinct phases. None
//! commits membership, issues a challenge, creates a signing key or starts work.
//! The product must call its original durable owner completion and attester
//! between `prove` and `deliver`; transport success is not that durable receipt.
//! The composition owns request dispatch, framing reset and one finite deadline
//! across all phases. Per-packet bounds must not extend that transaction deadline.
extern crate alloc;
mod basis;
mod observe;
mod protocol;
pub use basis::Basis;
use conduit_wire::CapabilityOfferImageRefusal;
pub use observe::{observe, Observation, ReceiveBuffers};
pub use protocol::{deliver, prove};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    Bounds,
    Scope,
    Encoding,
    Signature,
    Transport,
    Document,
    Allocation,
    Images(CapabilityOfferImageRefusal),
    Challenge,
    Receipt,
}
/// Independent receiver policy. Wire headers and member flash capacity cannot
/// enlarge these ceilings. Owned JSON contracts also require admitted hosted
/// preparation backing; these byte limits are not an allocator certificate.
#[derive(Clone, Copy)]
pub struct Limits {
    pub maximum_presentation_bytes: usize,
    pub maximum_document_chunks: usize,
    pub maximum_images: usize,
    pub maximum_image_bytes: usize,
    pub maximum_total_image_bytes: usize,
    pub maximum_expanded_offer_bytes: usize,
    pub maximum_total_expanded_bytes: usize,
}
impl Limits {
    fn valid(self) -> bool {
        self.maximum_presentation_bytes > 0
            && self.maximum_presentation_bytes <= 16 * 1024
            && self.maximum_document_chunks > 0
            && self.maximum_document_chunks <= 64
            && self.maximum_images > 0
            && self.maximum_images <= conduit_body::MAX_MEMBER_RETURN_IMAGES
            && self.maximum_image_bytes > 0
            && self.maximum_image_bytes <= self.maximum_total_image_bytes
            && self.maximum_total_image_bytes <= 24 * 1024
            && self.maximum_expanded_offer_bytes > 0
            && self.maximum_expanded_offer_bytes <= self.maximum_total_expanded_bytes
    }
}
#[cfg(test)]
mod tests;
