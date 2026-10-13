//! Browser execution uses the same reviewed prepared numeric emitter as std.
use super::{BrowserBack, BrowserInstallation};
use conduit_core::*;
use conduit_kernel::HostedValueStore;
pub(crate) const IMPLEMENTATION: &str = "browser/numeric-evaluation@1";
pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer: unreachable_offer,
    prepare,
    perform: None,
};
fn unreachable_offer() -> CapabilityOffer {
    panic!("numeric evaluation offers require a checked source Kind")
}
fn prepare(placement: &PlannedGear, _values: &mut HostedValueStore) -> Result<BrowserBack, String> {
    if placement.implementation_id.as_str() != IMPLEMENTATION {
        return Err("numeric browser implementation mismatch".into());
    }
    if placement
        .kind_id
        .as_str()
        .starts_with("conduitese/tensor/evaluate/")
    {
        return Ok(BrowserBack::installed_step(
            conduit_semantic_catalog::prepare_numeric_tensor(placement)?,
        ));
    }
    let output = conduit_semantic_catalog::prepare_numeric_evaluation(placement)?;
    Ok(BrowserBack::installed_step(
        conduit_semantic_catalog::NumericEvaluationBack::new(output),
    ))
}
pub(crate) fn offers(gear: &conduit_plot::CheckedGear) -> Result<Vec<CapabilityOffer>, String> {
    if gear
        .kind_id
        .as_str()
        .starts_with("conduitese/tensor/evaluate/")
    {
        let kind = conduit_semantic_catalog::numeric_tensor_kind(gear)?;
        return Ok(vec![conduit_semantic_catalog::numeric_tensor_offer(
            kind,
            IMPLEMENTATION,
            "browser/prepared-numeric-tensor@1",
        )]);
    }
    let kind = conduit_semantic_catalog::numeric_evaluation_kind(gear)?;
    Ok(conduit_semantic_catalog::NUMERIC_EVALUATION_ALGORITHMS
        .iter()
        .map(|algorithm| {
            conduit_semantic_catalog::numeric_evaluation_offer(
                kind.clone(),
                IMPLEMENTATION,
                "browser/prepared-numeric@1",
                *algorithm,
            )
        })
        .collect())
}

#[cfg(feature = "plot-runner")]
pub(crate) fn offer_for_placement(
    placement: &PlannedGear,
) -> Result<Option<CapabilityOffer>, String> {
    if placement.implementation_id.as_str() != IMPLEMENTATION {
        return Ok(None);
    }
    let tensor = placement
        .kind_id
        .as_str()
        .starts_with("conduitese/tensor/evaluate/");
    if tensor {
        conduit_plot::planned_gear_tensor_computations(placement)
            .map_err(|error| format!("Tensor source: {error:?}"))?;
    } else {
        conduit_plot::planned_gear_numeric_computation(placement)
            .map_err(|error| format!("numeric source: {error:?}"))?;
    }
    let selected = placement
        .realization_properties
        .iter()
        .find(|property| property.profile() == &numeric_realization_offer_profile())
        .ok_or("selected numeric offer property absent")?;
    let algorithm = NumericRealizationOffer::from_configuration(selected)
        .map_err(|error| format!("numeric profile: {error:?}"))?
        .algorithm;
    let kind = Kind {
        kind_id: placement.kind_id.clone(),
        kind_contract_revision: placement.kind_contract_revision.clone(),
        startup_parameters: placement
            .semantic_contract
            .configuration
            .iter()
            .map(|field| FrontStartupParameter {
                name: field.key.clone(),
                value_type: field.default_value.semantic_kind(),
                has_default: true,
            })
            .collect(),
        shorthand: None,
        inputs: placement.inputs.clone(),
        outputs: placement.outputs.clone(),
        configuration: placement.semantic_contract.configuration.clone(),
        semantic_laws: placement.semantic_contract.laws.clone(),
        limits: CapabilityLimits {
            max_active_instances: if tensor { 8 } else { 16 },
            max_queue_items: 1,
            max_queue_bytes: if tensor {
                conduit_semantic_catalog::MAXIMUM_NUMERIC_TENSOR_ENCODED_BYTES as u32
            } else {
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
            },
        },
    };
    let offer = if tensor {
        conduit_semantic_catalog::numeric_tensor_offer(
            kind,
            IMPLEMENTATION,
            "browser/prepared-numeric-tensor@1",
        )
    } else {
        conduit_semantic_catalog::numeric_evaluation_offer(
            kind,
            IMPLEMENTATION,
            "browser/prepared-numeric@1",
            algorithm,
        )
    };
    if placement.capability_id != offer.capability_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.execution_profile_id != offer.implementation.execution_profile_id
    {
        return Err("numeric placement does not match reviewed browser offer".into());
    }
    Ok(Some(offer))
}
