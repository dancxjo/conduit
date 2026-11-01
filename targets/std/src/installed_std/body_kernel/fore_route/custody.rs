//! Move the existing prepared slot into successful or stopped output custody.
use super::*;
use crate::body_execution::BodyForeDeliveryStage;

pub(super) fn deliver(
    scheduler: &mut InstalledScheduler,
    output: &mut Option<&mut dyn BodyForeOutputAdapter>,
    planned: &LoweredForePort,
    delivery: ExternalForeDelivery,
    borrowed_stream: bool,
) -> Result<ExternalForeDelivery, (BodyForePendingOutput, String)> {
    let mut stage = BodyForeDeliveryStage::NotAttempted;
    let result = (|| {
        let adapter = output
            .as_deref_mut()
            .ok_or("Body Fore output adapter lost")?;
        stage = BodyForeDeliveryStage::AdapterOutcomeUnknown;
        adapter.deliver(&delivery)?;
        stage = BodyForeDeliveryStage::AdapterAccepted;
        if borrowed_stream && !delivery.prepared_storage_is_unique() {
            return Err("borrowed Fore adapter retained kernel storage".into());
        }
        scheduler
            .remote_egress_accept(planned.endpoint, planned.cord, delivery.sequence)
            .map_err(|error| format!("Body Fore accept: {error:?}"))?;
        stage = BodyForeDeliveryStage::KernelAccepted;
        scheduler
            .remote_egress_delivered(planned.endpoint, planned.cord, delivery.sequence)
            .map_err(|error| format!("Body Fore acknowledge: {error:?}"))?;
        Ok::<_, String>(())
    })();
    match result {
        Ok(()) => Ok(delivery),
        Err(error) => Err((BodyForePendingOutput { delivery, stage }, error)),
    }
}
