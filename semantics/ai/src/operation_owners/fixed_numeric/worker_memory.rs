//! Exact worker-owned backing for the existing fixed Value vector realizations.
use super::*;
use crate::fixed_numeric_operations_back::{FixedConcatenateBack, FixedTanhBack};
use conduit_composite::PreparedWorkerBack;
fn tanh<const W: usize>(gear: &PlannedGear) -> Result<PreparedWorkerBack, String> {
    let fuel = u16::try_from(gear.inputs.len() + 1).map_err(|_| "worker fuel overflow")?;
    let back =
        FixedTanhBack::<W>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(gear, fuel)
            .map_err(|e| format!("worker tanh preparation: {e:?}"))?;
    let heap = back.worker_owned_heap_bytes();
    PreparedWorkerBack::new(Box::new(back), heap)
}
fn concatenate<const L: usize, const R: usize, const O: usize>(
    gear: &PlannedGear,
) -> Result<PreparedWorkerBack, String> {
    let fuel = u16::try_from(gear.inputs.len() + 1).map_err(|_| "worker fuel overflow")?;
    let back = FixedConcatenateBack::<L, R, O>::prepare_planned::<
        FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
    >(gear, fuel)
    .map_err(|e| format!("worker concatenate preparation: {e:?}"))?;
    let heap = back.worker_owned_heap_bytes();
    PreparedWorkerBack::new(Box::new(back), heap)
}
pub(super) fn prepare(gear: &PlannedGear) -> Result<PreparedWorkerBack, String> {
    match gear.kind_id.as_str() {
        "numeric/concatenate1x1" => concatenate::<1, 1, 2>(gear),
        "numeric/concatenate2x1" => concatenate::<2, 1, 3>(gear),
        "numeric/concatenate18x1" => concatenate::<18, 1, 19>(gear),
        "numeric/concatenate19x1" => concatenate::<19, 1, 20>(gear),
        "numeric/concatenate20x12" => concatenate::<20, 12, 32>(gear),
        "numeric/concatenate80x44" => concatenate::<80, 44, 124>(gear),
        "numeric/concatenate124x40" => concatenate::<124, 40, 164>(gear),
        "numeric/concatenate164x164" => concatenate::<164, 164, 328>(gear),
        "numeric/concatenate192x40" => concatenate::<192, 40, 232>(gear),
        "numeric/concatenate232x40" => concatenate::<232, 40, 272>(gear),
        "numeric/concatenate160x40" => concatenate::<160, 40, 200>(gear),
        "numeric/concatenate200x40" => concatenate::<200, 40, 240>(gear),
        "numeric/concatenate128x40" => concatenate::<128, 40, 168>(gear),
        "numeric/concatenate168x40" => concatenate::<168, 40, 208>(gear),
        "numeric/concatenate160x128" => concatenate::<160, 128, 288>(gear),
        "numeric/concatenate288x128" => concatenate::<288, 128, 416>(gear),
        "numeric/concatenate416x192" => concatenate::<416, 192, 608>(gear),
        "numeric/concatenate608x40" => concatenate::<608, 40, 648>(gear),
        "numeric/concatenate648x40" => concatenate::<648, 40, 688>(gear),
        "numeric/concatenate216x40" => concatenate::<216, 40, 256>(gear),
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
