//! Reconstruct only a pristine installed image's fixed initial evidence. This
//! neither births another identity nor restores mutable lifecycle history.
use alloc::string::String;
use conduit_body::{Body, BodyBiographyEvidence, BodyMembership, BodyWorkset};
use conduit_core::SignId;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PristineImageRefusal {
    Birth,
    Identity,
    Membership,
    Biography,
}
/// Representation boundary for trusted Make material. The expected Body was
/// already born by the construction host; all inputs retain that exact Sign,
/// sequence, name and workset. Ordinary installation/admission remains separate.
/// This accepts no current member, Wake, Plan, emergency policy or archive;
/// resumed Bodies must use their complete retained evidence and recovery path.
pub fn restore_pristine_image(
    workset: BodyWorkset,
    sequence: u64,
    sign: SignId,
    name: String,
    expected_body_id: &str,
) -> Result<BodyBiographyEvidence, PristineImageRefusal> {
    use PristineImageRefusal as E;
    let body = Body::born_with_plots(workset, sequence, sign).map_err(|_| E::Birth)?;
    if body.body_id.as_str() != expected_body_id {
        return Err(E::Identity);
    }
    let membership = BodyMembership::new(body.body_id.clone()).map_err(|_| E::Membership)?;
    BodyBiographyEvidence::born(body, membership, name).map_err(|_| E::Biography)
}
