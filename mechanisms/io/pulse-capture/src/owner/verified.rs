//! Exact verified selection and canonical codec preparation before transfer.
use super::*;
pub(crate) fn prepare_verified(
    table: &BaseCapabilityTable,
    claim: &BaseOperationClaim,
    attachment: PulseAttachment,
    contract: &PulseReadContract,
    selected: conduit_plan_lowering::selected_operation::SelectedVerifiedOperationPlan<'_>,
) -> Result<PreparedAssignedPulse, PulseCallRefusal> {
    check_possession(table, claim, attachment)?;
    let node = conduit_plan_lowering::selected_operation::bind_selected_verified_operation(
        table,
        claim,
        SelectedOperationContract {
            kind: contract.kind(),
            call: PULSE_READ_CALL,
            input_bytes: PULSE_READ_MAXIMUM_BYTES,
            output_bytes: PULSE_READ_MAXIMUM_BYTES,
            resource_bytes: attachment.resource_bytes,
        },
        selected,
    )
    .map_err(|reason| match reason {
        SelectedCallRefusal::WrongBinding => PulseCallRefusal::WrongBinding,
        SelectedCallRefusal::Possession => PulseCallRefusal::Possession,
    })?;
    Ok(PreparedAssignedPulse {
        node,
        codec: PreparedPulseCodec::prepare(contract).map_err(PulseCallRefusal::Canonical)?,
    })
}
