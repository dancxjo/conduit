//! Exact owned pointwise realizations for a separately selected compact DAG.
use super::*;
use crate::{
    fixed_numeric_compact::CompactNumericBack,
    fixed_numeric_operations_back::FixedTanhBack,
    fixed_numeric_signal_back::{FixedElementwiseBack, FixedElementwiseOperation},
};

impl FixedNumericOperationFactory {
    /// Prepare only explicit, owned, stateless Value operators. The caller must
    /// still install the exact Plan-selected DAG before Play. Without a binding
    /// these owners use their canonical boundary codecs.
    pub fn prepare_compact_reusable(
        &self,
        gear: &PlannedGear,
    ) -> Result<ReusableNumericBack, String> {
        self.selected(gear)?;
        if gear
            .inputs
            .iter()
            .chain(&gear.outputs)
            .any(|p| p.temporal != PortTemporal::Value)
            || !gear.host_calls.is_empty()
            || !gear.resources.is_empty()
        {
            return Err("compact pointwise owner requires effect-free Value ports".into());
        }
        let fuel =
            u16::try_from(gear.inputs.len() + 1).map_err(|_| "compact step fuel overflow")?;
        let kind = gear.kind_id.as_str();
        macro_rules! widths {
            ($($width:literal),+) => { $(
                if ["add", "multiply", "sigmoid", "complement", "tanh"].iter()
                    .any(|op| kind == format!("numeric/{op}{}", $width)) {
                    return prepare::<$width>(gear, fuel);
                }
            )+ };
        }
        widths!(4, 40, 64, 128, 160, 192, 320);
        Err("selected numeric operation has no compact pointwise realization".into())
    }
}
fn prepare<const W: usize>(gear: &PlannedGear, fuel: u16) -> Result<ReusableNumericBack, String> {
    let kind = gear.kind_id.as_str();
    if kind == format!("numeric/tanh{W}") {
        let owner =
            FixedTanhBack::<W>::prepare_planned::<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>(gear, fuel)
                .map_err(|error| format!("{error:?}"))?;
        return Ok(Box::new(
            CompactNumericBack::from_tanh(owner).map_err(|error| format!("{error:?}"))?,
        ));
    }
    for (name, operation) in [
        ("add", FixedElementwiseOperation::Add),
        ("multiply", FixedElementwiseOperation::Multiply),
        ("sigmoid", FixedElementwiseOperation::Sigmoid),
        ("complement", FixedElementwiseOperation::Complement),
    ] {
        if kind == format!("numeric/{name}{W}") {
            let owner = FixedElementwiseBack::<W>::prepare_planned::<
                FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
            >(gear, fuel, operation)
            .map_err(|error| format!("{error:?}"))?;
            return Ok(Box::new(
                CompactNumericBack::from_elementwise(owner)
                    .map_err(|error| format!("{error:?}"))?,
            ));
        }
    }
    Err("unsupported compact owner".into())
}
