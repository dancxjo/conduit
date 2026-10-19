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
        derive_planned_transformation_eligibility(gear)
            .and_then(|eligibility| eligibility.require(WorkTransformation::Fusion))
            .map_err(|error| format!("compact pointwise semantic eligibility: {error:?}"))?;
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

/// Host preparation metadata for an exact owned offer. Matching a name, empty
/// effects, or an F32-looking type alone never grants this realization.
pub fn compact_pointwise_offer_layout(
    offer: &CapabilityOffer,
) -> Result<conduit_kernel::scheduler::CompactF32Layout, String> {
    let name = offer.kind_id.as_str();
    if !crate::fixed_numeric_catalog::is_pure_pointwise(name) {
        return Err("Kind has no owned compact pointwise contract".into());
    }
    let expected = fixed_numeric_offer(name)?;
    let larger = crate::fixed_numeric_value_capacity::value_capacity64_profile(
        expected.implementation.implementation_id.as_str(),
    )?;
    let expected = if offer.implementation.execution_profile_id.as_str() == larger {
        fixed_numeric_offer_capacity64(name)?
    } else {
        expected
    };
    if *offer != expected {
        return Err("compact pointwise metadata requires the exact owned offer".into());
    }
    let mut kind =
        crate::fixed_numeric_catalog::fixed_numeric_contract(name)?.ok_or("owned Kind absent")?;
    if offer.implementation.execution_profile_id.as_str() == larger {
        kind = crate::fixed_numeric_value_capacity::value_capacity64_contract(kind)?;
    }
    let back = Back {
        capability_id: offer.capability_id.clone(),
        execution_profile_id: offer.implementation.execution_profile_id.clone(),
        implementation_id: offer.implementation.implementation_id.clone(),
        artifact_id: offer.implementation.artifact_id.clone(),
        host_calls: offer.host_calls.clone(),
        resource_requirements: offer.resource_requirements.clone(),
        authority_requirements: offer.authority_requirements.clone(),
    };
    derive_transformation_eligibility(&kind, &back)
        .and_then(|eligibility| eligibility.require(WorkTransformation::Fusion))
        .map_err(|error| format!("compact pointwise offer semantics: {error:?}"))?;
    let lanes = ["add", "multiply", "sigmoid", "complement", "tanh"]
        .iter()
        .find_map(|op| name.strip_prefix(&format!("numeric/{op}")))
        .ok_or("owned pointwise operation absent")?
        .parse::<u16>()
        .map_err(|_| "owned pointwise width absent")?;
    let output = offer.outputs.first().ok_or("pointwise output absent")?;
    let logical_bytes = offer
        .semantic_contract
        .value_contracts()
        .iter()
        .find_map(|contract| {
            (contract.location == FrontValueLocation::Output(output.port_id.clone()))
                .then_some(contract.contract.maximum_bytes)
        })
        .ok_or("pointwise output envelope absent")?;
    if lanes == 0 || u32::from(lanes) * 4 > logical_bytes {
        return Err("pointwise physical envelope invalid".into());
    }
    Ok(conduit_kernel::scheduler::CompactF32Layout {
        lanes,
        logical_bytes,
    })
}
