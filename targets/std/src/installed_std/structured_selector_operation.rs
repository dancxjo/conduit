//! Installed local execution for exact planned structured selectors.

use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    ConfigurationValue, PlannedGear, StructuredCanonicalSelection, StructuredSelector,
    StructuredSelectorRefusal, UnmatchedVariantDisposition, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::STRUCTURED_SELECTOR_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) use conduit_semantic_catalog::StructuredSelectorOperation;

pub(super) struct StructuredSelectorHost {
    selector: StructuredSelector,
    input_type: Vec<u8>,
    output_type: Vec<u8>,
    output: Vec<u8>,
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Result<Vec<Option<StructuredSelectorHost>>, String> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            if placement.implementation_id.as_str()
                == conduit_std_offers::STRUCTURED_SELECTOR_STD_IMPLEMENTATION
            {
                StructuredSelectorHost::from_placement(placement).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}

impl StructuredSelectorHost {
    pub(super) fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let selector = selector_from_placement(placement)?;
        validate_placement(placement, &selector)?;
        let input_type = selector
            .input_type()
            .canonical_bytes()
            .map_err(|error| format!("structured selector input type: {error:?}"))?;
        let output_type = selector
            .output_type()
            .canonical_bytes()
            .map_err(|error| format!("structured selector output type: {error:?}"))?;
        Ok(Self {
            selector,
            input_type,
            output_type,
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub(super) fn execute(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, StructuredSelectorRefusal> {
        match self.selector.select_canonical_into(
            input,
            &self.input_type,
            &self.output_type,
            &mut self.output,
        )? {
            StructuredCanonicalSelection::Matched => Ok(Some(&self.output)),
            StructuredCanonicalSelection::Unmatched(UnmatchedVariantDisposition::Drop) => Ok(None),
            StructuredCanonicalSelection::Unmatched(UnmatchedVariantDisposition::Refuse) => {
                Err(StructuredSelectorRefusal::UnmatchedVariant)
            }
        }
    }
}

fn selector_from_placement(placement: &PlannedGear) -> Result<StructuredSelector, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("structured selector requires one exact planned configuration".into());
    };
    let ("selector", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err("structured selector planned configuration is malformed".into());
    };
    StructuredSelector::from_canonical_hex(encoded)
        .map_err(|error| format!("structured selector configuration refusal: {error:?}"))
}

fn validate_placement(
    placement: &PlannedGear,
    selector: &StructuredSelector,
) -> Result<(), String> {
    let temporal = placement
        .inputs
        .first()
        .map(|port| port.temporal)
        .ok_or_else(|| "structured selector input is missing".to_string())?;
    let offer = conduit_std_offers::structured_selector_std_offer(selector, temporal);
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || placement.inputs[0].temporal != placement.outputs[0].temporal
    {
        return Err("planned structured selector differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    let selector = selector_from_placement(placement)?;
    validate_placement(placement, &selector)?;
    Ok(OperationBudget {
        value_items: 8,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 8) as u32,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    budget(placement)?;
    Ok(InstalledOperation::StructuredSelector(
        StructuredSelectorOperation::new(MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32),
    ))
}

pub(super) fn refusal_detail(refusal: &StructuredSelectorRefusal) -> u16 {
    match refusal {
        StructuredSelectorRefusal::WrongInputType => 1,
        StructuredSelectorRefusal::MalformedCheckedValue => 2,
        StructuredSelectorRefusal::UnmatchedVariant => 3,
        StructuredSelectorRefusal::CanonicalEncodingTooLarge => 4,
        _ => 5,
    }
}
