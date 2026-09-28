//! Portable Patchbay inspection of retained generated-output validation truth.

use conduit_presentation::{
    GeneratedValidationReceipt, PresentationDisclosure, PresentationDisclosureLevel,
    PresentationFragment, PresentationFragmentError, PresentationPropertyValue, PresentationRole,
};

use crate::portable_content::ContentBuilder;

/// Project one retained validator receipt without exposing generated prose or
/// asking Patchbay to reinterpret the validator's semantic judgment.
pub fn project_generated_validation_receipt(
    receipt: &GeneratedValidationReceipt,
    basis: conduit_presentation::PresentationContributionBasis,
) -> Result<PresentationFragment, PresentationFragmentError> {
    let mut content = ContentBuilder::new();
    let validation = content.subject_with_identity(
        format!("generated-validation/{}", receipt.receipt_identity()),
        PresentationRole::Semantic(conduit_core::KindId::from(
            "presentation/generated-validation-evidence",
        )),
        "Generated semantic validation",
    );
    for (name, value) in [
        ("receipt-identity", receipt.receipt_identity()),
        ("candidate-digest", receipt.candidate_digest()),
        ("assessment-identity", receipt.assessment_identity()),
        (
            "source-presentation",
            receipt.source_presentation_identity(),
        ),
        ("mask-identity", receipt.mask_identity()),
        ("mask-contract-revision", receipt.mask_contract_revision()),
        (
            "validator-implementation",
            receipt.validator_implementation_identity(),
        ),
        ("validator-back", receipt.validator_back_identity()),
        ("plan-id", receipt.plan_id().as_str()),
        ("placement-id", receipt.placement_id().as_str()),
        ("host-id", receipt.host_id().as_str()),
        ("boot-id", receipt.boot_id().as_str()),
    ] {
        content.property(
            &validation,
            name,
            PresentationPropertyValue::Identity(value.into()),
        );
    }
    content.property(
        &validation,
        "source-presentation-revision",
        PresentationPropertyValue::Count(receipt.source_presentation_revision()),
    );
    content.property(
        &validation,
        "accepted-correlation-count",
        PresentationPropertyValue::Count(receipt.accepted_correlations().len() as u64),
    );
    content.property(
        &validation,
        "validation-disposition",
        PresentationPropertyValue::Text(format!("{:?}", receipt.disposition()).to_lowercase()),
    );
    if let Some(code) = receipt.terminal_code() {
        content.property(
            &validation,
            "terminal-code",
            PresentationPropertyValue::Text(code.into()),
        );
    }
    let fragment = PresentationFragment {
        basis,
        subjects: content.subjects,
        relationships: content.relationships,
        composition: Vec::new(),
        properties: content.properties,
        text: content.text,
        actions: Vec::new(),
        disclosures: vec![PresentationDisclosure {
            subject: validation,
            level: PresentationDisclosureLevel::ExactProvenance,
        }],
        temporal_references: Vec::new(),
        temporal_facts: Vec::new(),
    };
    fragment.validate_bounds()?;
    Ok(fragment)
}
