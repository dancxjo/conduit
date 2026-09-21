//! Exact finite normalized-pattern comparison offer.

use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId,
};

pub const COMPARE_PATTERN_BROWSER_PROFILE: &str =
    "browser/compare-normalized-pattern-kernel-hosted@1";
pub const COMPARE_PATTERN_BROWSER_IMPLEMENTATION: &str =
    "browser/kernel-compare-normalized-pattern@1";
pub const COMPARE_PATTERN_BROWSER_ARTIFACT: &str =
    "conduit-browser-runtime/compare-normalized-pattern@1";
pub const COMPARE_PATTERN_CANDIDATE_OPERATION: &str = "conduit.host/compare-pattern-candidate@1";
pub const COMPARE_PATTERN_TEMPLATE_OPERATION: &str = "conduit.host/compare-pattern-template@1";

pub fn offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::compare_normalized_pattern_semantic_contract();
    let kind_id = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from("compare-normalized-pattern"),
            execution_profile_id: ExecutionProfileId::from(COMPARE_PATTERN_BROWSER_PROFILE),
            implementation_id: ImplementationId::from(COMPARE_PATTERN_BROWSER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(COMPARE_PATTERN_BROWSER_ARTIFACT),
            host_calls: vec![
                host_call(COMPARE_PATTERN_CANDIDATE_OPERATION, &kind_id),
                host_call(COMPARE_PATTERN_TEMPLATE_OPERATION, &kind_id),
            ],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn host_call(contract: &str, kind: &conduit_core::KindId) -> HostCallRequirement {
    HostCallRequirement {
        contract_id: HostCallContractId::from(contract),
        target_kind: Some(kind.clone()),
        maximum_in_flight: 1,
        maximum_input_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        maximum_output_bytes: super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
    }
}

pub(super) fn prepare(
    placement: &conduit_core::PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<super::BrowserOperation, String> {
    tolerance(placement)?;
    Ok(super::BrowserOperation::installed_step(
        conduit_semantic_catalog::PatternComparisonOperation::new(
            super::MAXIMUM_BROWSER_VALUE_BYTES as u32,
        ),
    ))
}

fn tolerance(placement: &conduit_core::PlannedGear) -> Result<u64, String> {
    use conduit_core::ConfigurationValue;
    super::factory::validate_placement(placement, &offer())?;
    if placement.configuration.len() != 2 {
        return Err("comparison requires exact metric and tolerance".into());
    }
    let metric = placement
        .configuration
        .iter()
        .find(|item| item.key == "metric");
    if !matches!(metric.map(|item| &item.value), Some(ConfigurationValue::Text(metric))
        if metric == conduit_semantic_catalog::MAXIMUM_ABSOLUTE_METRIC)
    {
        return Err("unsupported pattern comparison metric".into());
    }
    match placement
        .configuration
        .iter()
        .find(|item| item.key == "tolerance-millionths")
        .map(|item| &item.value)
    {
        Some(ConfigurationValue::U64(value))
            if *value <= conduit_semantic_catalog::NORMALIZED_SCALE =>
        {
            Ok(*value)
        }
        _ => Err("pattern comparison tolerance out of range".into()),
    }
}

pub(crate) fn prepare_codec(
    placement: &conduit_core::PlannedGear,
) -> Result<Option<conduit_semantic_catalog::BoundedPatternComparisonCodec>, String> {
    if placement.implementation_id.as_str() != COMPARE_PATTERN_BROWSER_IMPLEMENTATION {
        return Ok(None);
    }
    conduit_semantic_catalog::BoundedPatternComparisonCodec::new(tolerance(placement)?).map(Some)
}

pub(super) static INSTALLATION: super::factory::BrowserInstallation =
    super::factory::BrowserInstallation {
        implementation_id: COMPARE_PATTERN_BROWSER_IMPLEMENTATION,
        offer,
        prepare,
        perform: None,
    };
pub(crate) fn input_port(
    contract: &str,
) -> Option<conduit_semantic_catalog::PatternComparisonInput> {
    use conduit_semantic_catalog::PatternComparisonInput;
    match contract {
        COMPARE_PATTERN_CANDIDATE_OPERATION => Some(PatternComparisonInput::Candidate),
        COMPARE_PATTERN_TEMPLATE_OPERATION => Some(PatternComparisonInput::Template),
        _ => None,
    }
}
pub(crate) fn failure(
    refusal: conduit_semantic_catalog::PatternComparisonRefusal,
) -> conduit_kernel::Failure {
    use conduit_semantic_catalog::PatternComparisonRefusal::*;
    conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidInput,
        detail: match refusal {
            Malformed => 1,
            UnsupportedMetric => 2,
            ToleranceOutOfRange => 3,
            AlgorithmMismatch => 4,
            LengthMismatch => 5,
        },
    }
}
