//! Hosted identity validation for the shared prepared `flow/zip` Back.
use super::back::{BackBudget, BackFactory, InstalledBack};
pub(super) use conduit_composite::FlowZipBack;
use conduit_core::{CheckedValueContract, FrontValueLocation, PlannedGear};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::FLOW_ZIP_IMPLEMENTATION,
    budget,
    prepare,
};

fn exact_contracts(
    placement: &PlannedGear,
) -> Result<
    (
        &CheckedValueContract,
        &CheckedValueContract,
        &CheckedValueContract,
    ),
    String,
> {
    let contracts = placement.semantic_contract.value_contracts();
    let at = |location| {
        contracts
            .iter()
            .find(|entry| entry.location == location)
            .map(|entry| &entry.contract)
    };
    Ok((
        at(FrontValueLocation::Input(conduit_core::port_id("left")))
            .ok_or("flow/zip placement has no exact left value contract")?,
        at(FrontValueLocation::Input(conduit_core::port_id("right")))
            .ok_or("flow/zip placement has no exact right value contract")?,
        at(FrontValueLocation::Output(conduit_core::port_id("paired")))
            .ok_or("flow/zip placement has no exact paired value contract")?,
    ))
}

fn validate(
    placement: &PlannedGear,
) -> Result<
    (
        &CheckedValueContract,
        &CheckedValueContract,
        &CheckedValueContract,
    ),
    String,
> {
    let (left, right, paired) = exact_contracts(placement)?;
    let expected = conduit_semantic_catalog::flow_zip_semantic_contract(left, right)
        .map_err(str::to_string)?;
    if left.maximum_bytes > conduit_std_offers::FLOW_ZIP_MAXIMUM_INPUT_BYTES
        || right.maximum_bytes > conduit_std_offers::FLOW_ZIP_MAXIMUM_INPUT_BYTES
        || placement.kind_id.as_str() != conduit_semantic_catalog::FLOW_ZIP_KIND
        || placement.kind_contract_revision.as_str()
            != conduit_semantic_catalog::FLOW_ZIP_CONTRACT_REVISION
        || placement.execution_profile_id.as_str() != conduit_std_offers::FLOW_ZIP_EXECUTION_PROFILE
        || placement.implementation_id.as_str() != conduit_std_offers::FLOW_ZIP_IMPLEMENTATION
        || placement.artifact_id.as_str() != conduit_std_offers::FLOW_ZIP_ARTIFACT
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
    {
        return Err("planned flow/zip identity differs from its exact specialization".into());
    }
    Ok((left, right, paired))
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let (left, right, paired) = validate(placement)?;
    Ok(BackBudget {
        value_items: 3,
        value_bytes: left
            .maximum_bytes
            .checked_add(right.maximum_bytes)
            .and_then(|bytes| bytes.checked_add(paired.maximum_bytes))
            .ok_or("flow/zip prepared value budget overflows")?,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: paired.maximum_bytes,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let (left, right, _) = validate(placement)?;
    let back = FlowZipBack::prepare(left, right)
        .map_err(|error| format!("cannot prepare flow/zip pair encoder: {error:?}"))?;
    Ok(InstalledBack::FlowZip(back))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, TEXT_INFO_ID};
    use conduit_kernel::{scheduler::StepBack, PortId};

    #[test]
    fn closed_installed_inventory_preserves_both_terminal_contracts() {
        let left = CheckedValueContract {
            value_kind: kind_id(TEXT_INFO_ID),
            maximum_bytes: 16,
            constraints: vec![],
        };
        let right = CheckedValueContract {
            value_kind: kind_id("value/count"),
            maximum_bytes: 8,
            constraints: vec![],
        };
        let installed = InstalledBack::FlowZip(FlowZipBack::prepare(&left, &right).unwrap());
        let contracts = <InstalledBack as StepBack<2>>::terminal_transductions(&installed);
        assert_eq!(contracts[0].unwrap().input, PortId(0));
        assert_eq!(contracts[1].unwrap().input, PortId(1));
    }
}
