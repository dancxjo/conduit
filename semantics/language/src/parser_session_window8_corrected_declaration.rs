//! Allocation-free origin-corrected411 declaration comparison.
//! This distinct candidate preserves the old baseline declaration unchanged. This is only one gate in
//! the sealed Session factory: complete expected signature family admission,
//! actual source/model execution and opaque revision provenance remain required.
use crate::lexical_proposer_port::token_producer::model_definition::{
    ProposalModelDefinition, ProposalSegmentationAbi,
};
use conduit_core::BoundedResourceRef;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Window8DeclarationRefusal {
    Policy,
    Artifact,
    Source,
    Signature,
}
pub(crate) const EXPECTED_SIGNATURE: &[u8] = include_bytes!(
    "../training/ewt_proposal_v2/origin-corrected-v2/expected-metadata/signature.native.bin"
);
pub(crate) fn check_fixed_declaration(
    d: &ProposalModelDefinition,
) -> Result<(), Window8DeclarationRefusal> {
    use Window8DeclarationRefusal as R;
    if core::char::UNICODE_VERSION != (17, 0, 0)
        || d.identity() != "TRAIN+22-reviewed-proposal-candidate2-origin-corrected-v2"
        || d.feature_abi() != "selected-history-origin/411-27-76@2"
        || d.dimensions() != (411, 76, 27)
        || d.maximum_score_magnitude() != 1_000_000
        || d.training_manifest() != include_bytes!("../training/ewt_proposal_v2/origin-corrected-v2/declared-model-training-manifest.json")
        || d.canonical_proposer_definition() != include_bytes!("../training/ewt_proposal_v2/origin-corrected-v2/expected-metadata/proposer-definition.native.bin")
    { return Err(R::Policy); }
    if d.segmentation_abi() != ProposalSegmentationAbi::UnicodeScalarAlphanumericApostropheV1
        || d.proposal_source_material() != include_bytes!("../lexical_proposer.conduit")
        || d.dictionary_identity()
            != [
                18, 69, 142, 216, 90, 98, 255, 25, 167, 148, 168, 247, 158, 36, 255, 140, 126, 118,
                227, 151, 83, 146, 55, 158, 211, 186, 158, 213, 201, 236, 137, 254,
            ]
    {
        return Err(R::Policy);
    }
    let a = d.artifact();
    let expected = BoundedResourceRef::validate_encoded(include_bytes!(
        "../training/ewt_proposal_v2/origin-corrected-v2/expected-metadata/artifact-reference.bin"
    ))
    .map_err(|_| R::Artifact)?;
    if a.architecture_profile != "ai/categorical-linear@1"
        || a.format_profile != "model/categorical-i16-sum@1"
        || a.precision_profile != "number/i16-weights-i64-sums@1"
        || a.state_schema_version != 1
        || a.content.identity != expected.identity
        || a.content.lifetime.version != expected.version
        || a.content.content_profile.0 != expected.content_profile
        || a.content.access_class.0 != expected.access_class
        || a.content.extent != expected.extent
        || expected.has_expiry
        || a.content.lifetime.expires_at.is_some()
    {
        return Err(R::Artifact);
    }
    if a.signature_identity
        != [
            1, 123, 37, 87, 45, 164, 87, 190, 204, 20, 70, 31, 196, 56, 18, 226, 130, 238, 59, 82,
            162, 101, 17, 160, 65, 115, 230, 211, 113, 184, 7, 28,
        ]
    {
        return Err(R::Signature);
    }
    let source = d.source_contract();
    if source.action_contract
        != [
            235, 217, 53, 61, 150, 129, 92, 141, 42, 119, 48, 38, 129, 158, 150, 221, 11, 115, 12,
            98, 81, 150, 151, 85, 92, 142, 104, 207, 76, 170, 73, 133,
        ]
    {
        return Err(R::Source);
    }
    if source.availability_contract
        != [
            221, 254, 192, 171, 82, 204, 54, 137, 189, 45, 151, 18, 136, 198, 59, 227, 113, 21,
            112, 163, 243, 244, 245, 31, 178, 114, 195, 164, 169, 119, 1, 250,
        ]
    {
        return Err(R::Source);
    }
    if source.feature_contract
        != [
            152, 23, 152, 240, 199, 184, 160, 30, 34, 107, 34, 60, 142, 3, 8, 137, 41, 209, 91,
            147, 149, 23, 248, 95, 71, 17, 167, 197, 177, 39, 46, 16,
        ]
    {
        return Err(R::Source);
    }
    if source.joint_choice_contract
        != [
            67, 221, 12, 62, 153, 8, 118, 122, 249, 208, 8, 59, 167, 234, 253, 216, 20, 166, 250,
            38, 213, 213, 223, 111, 162, 56, 86, 144, 23, 154, 39, 110,
        ]
    {
        return Err(R::Source);
    }
    if source.numeric_indices_contract
        != [
            14, 247, 150, 12, 59, 54, 143, 63, 37, 134, 203, 245, 84, 186, 214, 28, 57, 101, 84,
            34, 108, 77, 167, 196, 138, 135, 247, 123, 30, 200, 138, 154,
        ]
    {
        return Err(R::Source);
    }
    if source.numeric_scores_contract
        != [
            55, 190, 172, 157, 61, 158, 212, 65, 104, 74, 53, 219, 76, 193, 130, 82, 55, 41, 89,
            141, 222, 40, 35, 236, 167, 134, 213, 211, 157, 106, 131, 241,
        ]
    {
        return Err(R::Source);
    }
    // The complete fresh expected-signature Native equality gate is performed
    // separately under its family conversion reservation. Digest comparison here
    // does not stand in for that gate.
    Ok(())
}
