//! Exact worker-owned backing for the existing fixed tanh realizations.
use super::*;
use crate::fixed_numeric_operations_back::FixedTanhBack;
use conduit_composite::PreparedWorkerBack;
fn tanh<const W: usize>(gear: &PlannedGear) -> Result<PreparedWorkerBack, String> {
    let fuel = u16::try_from(gear.inputs.len() + 1).map_err(|_| "worker fuel overflow")?;
    let back =
        FixedTanhBack::<W>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(gear, fuel)
            .map_err(|e| format!("worker tanh preparation: {e:?}"))?;
    let heap = back.worker_owned_heap_bytes();
    PreparedWorkerBack::new(Box::new(back), heap)
}
pub(super) fn prepare(gear: &PlannedGear) -> Result<PreparedWorkerBack, String> {
    match gear.kind_id.as_str() {
        "numeric/tanh4" => tanh::<4>(gear),
        "numeric/tanh40" => tanh::<40>(gear),
        "numeric/tanh64" => tanh::<64>(gear),
        "numeric/tanh128" => tanh::<128>(gear),
        "numeric/tanh160" => tanh::<160>(gear),
        "numeric/tanh192" => tanh::<192>(gear),
        "numeric/tanh320" => tanh::<320>(gear),
        _ => Err("numeric operation has no admitted worker memory receipt".into()),
    }
}
