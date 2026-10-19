//! Exact owned matrix transport metadata and admitted-resource preparation.
use super::*;
use crate::{
    fixed_numeric_compact_linear::CompactLinearBack,
    fixed_numeric_linear_back::{FixedLinearBack, LINEAR_IMPLEMENTATION},
};
use conduit_kernel::{scheduler::CompactF32Layout, PortId as KernelPortId};

/// Physical port metadata only. Tensor authority/content admission remains in
/// the existing resource and linear owners; matching an offer grants no access.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactLinearOfferLayout {
    pub numeric_input: (KernelPortId, CompactF32Layout),
    pub output: (KernelPortId, CompactF32Layout),
    pub canonical_inputs: [KernelPortId; 1],
}
pub fn compact_linear_offer_layout(
    offer: &CapabilityOffer,
) -> Result<CompactLinearOfferLayout, String> {
    let name = offer.kind_id.as_str();
    let expected = fixed_numeric_offer(name)?;
    if *offer != expected
        || offer.implementation.implementation_id.as_str() != LINEAR_IMPLEMENTATION
        || !offer
            .kind_contract_revision
            .as_str()
            .ends_with("/read-immutable-input@1")
    {
        return Err("compact linear metadata requires the exact reviewed owned offer".into());
    }
    let kind = crate::fixed_numeric_catalog::fixed_numeric_contract(name)?
        .ok_or("owned linear Kind absent")?;
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
        .map_err(|e| format!("compact linear offer semantics: {e:?}"))?;
    // Parsing follows complete owned-offer equality, not name-based admission.
    let (input, output) = name
        .strip_prefix("numeric/linear")
        .and_then(|s| s.split_once('x'))
        .ok_or("owned linear shape absent")?;
    let layout = |width: &str, location: FrontValueLocation| -> Result<CompactF32Layout, String> {
        let lanes = width
            .parse::<u16>()
            .map_err(|_| "owned linear width invalid")?;
        let logical_bytes = offer
            .semantic_contract
            .value_contracts()
            .iter()
            .find_map(|c| (c.location == location).then_some(c.contract.maximum_bytes))
            .ok_or("owned linear port envelope absent")?;
        if lanes == 0 || lanes > 1024 || u32::from(lanes) * 4 > logical_bytes {
            return Err("owned linear physical envelope invalid".into());
        }
        Ok(CompactF32Layout {
            lanes,
            logical_bytes,
        })
    };
    Ok(CompactLinearOfferLayout {
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
pub(super) fn prepare(
    gear: &PlannedGear,
    fuel: u16,
    bindings: &TensorBindings,
) -> Result<ReusableNumericBack, String> {
    macro_rules! shape {
        ($i:literal,$o:literal) => {
            if gear.kind_id.as_str()
                == concat!("numeric/linear", stringify!($i), "x", stringify!($o))
            {
                let owner = FixedLinearBack::<$i, $o>::prepare_planned_owned::<
                    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
                >(gear, fuel, resource(bindings, "weights")?)
                .map_err(|e| format!("{e:?}"))?;
                return Ok(Box::new(
                    CompactLinearBack::from_linear(owner).map_err(|e| format!("{e:?}"))?,
                ));
            }
        };
    }
    shape!(3, 2);
    shape!(161, 18);
    shape!(18, 18);
    shape!(80, 1);
    shape!(328, 192);
    shape!(192, 192);
    shape!(192, 4);
    shape!(272, 480);
    shape!(160, 480);
    shape!(240, 384);
    shape!(128, 384);
    shape!(208, 384);
    shape!(160, 160);
    shape!(128, 128);
    shape!(688, 128);
    shape!(128, 40);
    Err("selected linear Kind has no reviewed compact realization".into())
}
