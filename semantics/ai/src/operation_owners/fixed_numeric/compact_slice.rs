//! Exact owned slice transport metadata and explicit-index preparation.
use super::*;
use crate::fixed_numeric_index_back::INDEX_IMPLEMENTATION;
use conduit_kernel::{scheduler::CompactF32Layout, PortId as KernelPortId};

/// Physical port metadata only. The start port retains its canonical checked
/// integer codec and the original owner checks slice bounds on every invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactSliceOfferLayout {
    pub numeric_input: (KernelPortId, CompactF32Layout),
    pub output: (KernelPortId, CompactF32Layout),
    pub canonical_inputs: [KernelPortId; 1],
}
pub fn compact_slice_offer_layout(
    offer: &CapabilityOffer,
) -> Result<CompactSliceOfferLayout, String> {
    let name = offer.kind_id.as_str();
    if !crate::fixed_numeric_catalog::is_pure_slice(name) {
        return Err("unreviewed compact slice".into());
    }
    let expected = if offer.implementation.execution_profile_id.as_str()
        == crate::fixed_numeric_value_capacity::value_capacity64_profile(INDEX_IMPLEMENTATION)?
    {
        fixed_numeric_offer_capacity64(name)?
    } else {
        fixed_numeric_offer(name)?
    };
    if *offer != expected
        || offer.implementation.implementation_id.as_str() != INDEX_IMPLEMENTATION
        || !offer
            .kind_contract_revision
            .as_str()
            .ends_with("/pure-explicit-slice@1")
    {
        return Err("compact slice metadata requires the exact reviewed owned offer".into());
    }
    let mut kind = crate::fixed_numeric_catalog::fixed_numeric_contract(name)?
        .ok_or("owned slice Kind absent")?;
    if offer.limits.max_active_instances == 64 {
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
        .and_then(|e| e.require(WorkTransformation::Fusion))
        .map_err(|e| format!("compact slice offer semantics: {e:?}"))?;
    // Parsing follows complete owned-offer equality, not name-based admission.
    let (input, output) = name
        .strip_prefix("numeric/slice")
        .and_then(|s| s.split_once('x'))
        .ok_or("owned slice shape absent")?;
    let layout = |width: &str, location: FrontValueLocation| -> Result<CompactF32Layout, String> {
        let lanes = width
            .parse::<u16>()
            .map_err(|_| "owned slice width invalid")?;
        let logical_bytes = offer
            .semantic_contract
            .value_contracts()
            .iter()
            .find_map(|c| (c.location == location).then_some(c.contract.maximum_bytes))
            .ok_or("owned slice port envelope absent")?;
        if lanes == 0 || lanes > 1024 || u32::from(lanes) * 4 > logical_bytes {
            return Err("owned slice physical envelope invalid".into());
        }
        Ok(CompactF32Layout {
            lanes,
            logical_bytes,
        })
    };
    Ok(CompactSliceOfferLayout {
        numeric_input: (
            KernelPortId(0),
            layout(
                input,
                FrontValueLocation::Input(offer.inputs[0].port_id.clone()),
            )?,
        ),
        output: (
            KernelPortId(0),
            layout(
                output,
                FrontValueLocation::Output(offer.outputs[0].port_id.clone()),
            )?,
        ),
        canonical_inputs: [KernelPortId(1)],
    })
}

pub(super) fn prepare(gear: &PlannedGear, fuel: u16) -> Result<ReusableNumericBack, String> {
    use crate::{
        fixed_numeric_compact_slice::CompactSliceBack,
        fixed_numeric_index_back::{FixedIndexBack, FixedIndexOperation},
    };
    macro_rules! shape {
        ($i:literal,$o:literal) => {
            if gear.kind_id.as_str()
                == concat!("numeric/slice", stringify!($i), "x", stringify!($o))
            {
                let owner = FixedIndexBack::<$i, $o>::prepare_planned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, FixedIndexOperation::Slice)
                .map_err(|e| format!("{e:?}"))?;
                return Ok(Box::new(
                    CompactSliceBack::from_slice(owner).map_err(|e| format!("{e:?}"))?,
                ));
            }
        };
    }
    shape!(480, 160);
    shape!(384, 128);
    Err("unreviewed compact slice".into())
}
