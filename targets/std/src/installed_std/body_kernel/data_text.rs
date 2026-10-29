//! One selected file provider per exact local partition, prepared before Play.
use super::super::data_text_host::DataTextGenerationHosts;
use conduit_core::{HostCallContractId, PlanFragment};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn is_file(contract: &HostCallContractId) -> bool {
    [
        conduit_std_offers::DURABLE_TEXT_PUBLISH_CALL,
        conduit_std_offers::DURABLE_TEXT_READ_CALL,
    ]
    .contains(&contract.as_str())
}

pub(super) fn prepare<'a>(
    fragments: &[&PlanFragment],
    lowered: &[LoweredPlanFragment],
    #[cfg(target_os = "linux")] selected_file: Option<
        &'a crate::text_file::SelectedTextFileResidence,
    >,
) -> Result<Vec<DataTextGenerationHosts<'a>>, String> {
    if fragments.len() != lowered.len() {
        return Err("Body file Text partition mismatch".into());
    }
    fragments
        .iter()
        .zip(lowered)
        .map(|(fragment, partition)| {
            DataTextGenerationHosts::prepare_for_body(
                fragment,
                &partition.identity,
                #[cfg(target_os = "linux")]
                selected_file,
            )
        })
        .collect()
}
