//! Lower immutable Core-verified metadata without reconstituting configurations.
use super::*;
/// Flat preparation entrance. Core has checked full canonical records and all
/// ordinary whole-Plan structure; unsupported activation ownership was refused.
/// Resource possession and Body/Play authority remain separate admissions.
pub fn lower_verified_plan_metadata_fragment(
    plan: &conduit_core::VerifiedPlanMetadata,
    fragment_id: &FragmentId,
) -> Result<LoweredPlanFragment, LoweringError> {
    lower_verified_plan_metadata_fragment_for_profile(
        plan,
        fragment_id,
        FIXED_KERNEL_STORAGE_PROFILE,
    )
}
pub fn lower_verified_plan_metadata_fragment_for_profile(
    plan: &conduit_core::VerifiedPlanMetadata,
    fragment_id: &FragmentId,
    profile: KernelStorageProfile,
) -> Result<LoweredPlanFragment, LoweringError> {
    let view = plan
        .fragment(fragment_id)
        .ok_or(LoweringError::InvalidFragment)?;
    let fragment = view.lowering_metadata();
    admission::validate_verified_fragment(fragment, profile)?;
    lower_verified_fragment_for_profile(fragment, profile)
}
