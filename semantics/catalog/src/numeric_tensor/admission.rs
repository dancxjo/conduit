//! Source, offer and Plan readmission for the finite Tensor consumer.
use super::*;
#[cfg(all(feature = "kernel-step", feature = "plot-catalog"))]
use conduit_plot::rust_binding::NativeRustBinding;

#[cfg(feature = "plot-catalog")]
pub fn install_tensor_evaluation(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::{string::ToString, vec};
    let scalar = StructuredInfoType::leaf(kind_id(MATH_VALUE_INFO_ID))
        .map_err(|value| alloc::format!("{value:?}"))?;
    let count = StructuredInfoType::leaf(kind_id(COUNT_INFO_ID))
        .map_err(|value| alloc::format!("{value:?}"))?;
    let axis = TensorAxis::semantic_type().map_err(|value| alloc::format!("{value:?}"))?;
    for (name, value) in [
        (
            "NumericTensorCoordinates",
            StructuredInfoType::bounded_sequence(scalar, 1, 8)
                .map_err(|value| alloc::format!("{value:?}"))?,
        ),
        (
            "NumericTensorDimensions",
            StructuredInfoType::bounded_sequence(count, 1, 8)
                .map_err(|value| alloc::format!("{value:?}"))?,
        ),
        (
            "NumericTensorAxes",
            StructuredInfoType::bounded_sequence(axis, 1, 8)
                .map_err(|value| alloc::format!("{value:?}"))?,
        ),
    ] {
        startup.insert_structured_type(name, value)?;
    }
    startup.insert(conduit_plot::KindSignature {
        kind: "tensor/evaluate".into(),
        startup_parameters: [
            ("values", "NumericTensorCoordinates"),
            ("dimensions", "NumericTensorDimensions"),
            ("axes", "NumericTensorAxes"),
            ("requirement", "NumericRequirement"),
        ]
        .into_iter()
        .map(
            |(name, value_type)| conduit_plot::StartupParameterSignature {
                name: name.into(),
                value_type: value_type.into(),
                default: None,
            },
        )
        .collect(),
    })?;
    profile
        .insert(conduit_plot::KindProjection {
            kind_id: kind_id("tensor/evaluate"),
            kind_contract_revision: KindIdentity::from("conduit.tensor/evaluate-source@1"),
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("result"),
                value_kind: kind_id(conduit_data::TENSOR_INFO_ID),
                direction: PortDirection::Output,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            }],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(feature = "plot-catalog")]
pub fn numeric_tensor_kind(
    gear: &conduit_plot::CheckedGear,
) -> Result<Kind, alloc::string::String> {
    conduit_plot::checked_gear_tensor_computations(gear)
        .map_err(|error| alloc::format!("Tensor source: {error:?}"))?;
    let kind = Kind {
        kind_id: gear.kind_id.clone(),
        kind_contract_revision: gear.kind_contract_revision.clone(),
        startup_parameters: gear.startup_parameters.clone(),
        shorthand: None,
        inputs: gear.inputs.clone(),
        outputs: gear.outputs.clone(),
        configuration: gear.semantic_contract.configuration.clone(),
        semantic_laws: gear.semantic_contract.laws.clone(),
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_NUMERIC_TENSOR_ENCODED_BYTES as u32,
        },
    };
    kind.validate()
        .map_err(|error| alloc::format!("Tensor Kind: {error:?}"))?;
    Ok(kind)
}
pub fn numeric_tensor_offer(kind: Kind, implementation: &str, profile: &str) -> CapabilityOffer {
    let mut offer = crate::numeric_evaluation_offer::numeric_evaluation_offer(
        kind,
        implementation,
        profile,
        NumericAlgorithm::OrderedSoftwareBinary64V1,
    );
    // Eight concurrent retained coordinate certificates share one finite reservation.
    offer.resource_requirements = alloc::vec![resource_requirement(
        RUNTIME_MEMORY_RESOURCE_CLASS,
        8_388_608
    )];
    offer
}
#[cfg(all(feature = "kernel-step", feature = "plot-catalog"))]
pub fn prepare_numeric_tensor(
    placement: &PlannedGear,
) -> Result<NumericTensorBack, alloc::string::String> {
    use alloc::format;
    if placement.artifact_id != numeric_engine_artifact_id() {
        return Err("Tensor implementation artifact is not reviewed".into());
    }
    let bundle = conduit_plot::planned_gear_tensor_computations(placement)
        .map_err(|error| format!("Tensor source custody: {error:?}"))?;
    let one = |profile: &KindId| {
        let mut values = placement
            .realization_properties
            .iter()
            .filter(|property| property.profile() == profile);
        let value = values
            .next()
            .ok_or_else(|| "Tensor selected realization property absent".into());
        if values.next().is_some() {
            return Err(alloc::string::String::from(
                "Tensor selected realization property duplicated",
            ));
        }
        value
    };
    let offer =
        NumericRealizationOffer::from_configuration(one(&numeric_realization_offer_profile())?)
            .map_err(|error| format!("Tensor algorithm: {error:?}"))?;
    verify_numeric_tensor_witness_configuration(
        one(&numeric_tensor_witness_profile())?,
        &bundle.coordinates,
        &bundle.requirement,
    )
    .map_err(|error| format!("Tensor witness readmission: {error:?}"))?;
    let dimensions = StructuredInfoValue::from_canonical_bytes(bundle.dimensions.canonical_value())
        .map_err(|error| format!("Tensor dimensions: {error:?}"))?;
    let StructuredInfoValueShape::Collection(dimensions) = dimensions.shape() else {
        return Err("Tensor dimensions Type mismatch".into());
    };
    let dimensions: Vec<u64> = dimensions
        .iter()
        .map(|value| match value.shape() {
            StructuredInfoValueShape::Leaf(bytes) => bytes
                .try_into()
                .map(u64::from_le_bytes)
                .map_err(|_| "Tensor dimension encoding".into()),
            _ => Err(alloc::string::String::from(
                "Tensor dimension Type mismatch",
            )),
        })
        .collect::<Result<_, _>>()?;
    let axes = StructuredInfoValue::from_canonical_bytes(bundle.axes.canonical_value())
        .map_err(|error| format!("Tensor axes: {error:?}"))?;
    let StructuredInfoValueShape::Collection(axes) = axes.shape() else {
        return Err("Tensor axes Type mismatch".into());
    };
    let axes = axes
        .iter()
        .map(|value| {
            TensorAxis::from_structured(value.clone())
                .map_err(|error| format!("Tensor axis: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let computations = bundle
        .coordinates
        .into_iter()
        .map(|graph| (graph, bundle.requirement.clone()))
        .collect::<Vec<_>>();
    let projection = ProjectedTensorF64::prepare(&dimensions, &axes, &computations, &offer)
        .map_err(|error| format!("Tensor projection: {error:?}"))?;
    let reserved = placement
        .resources
        .iter()
        .filter(|binding| binding.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS)
        .try_fold(0u32, |sum, binding| {
            sum.checked_add(binding.units)
                .ok_or("Tensor resource reservation overflow")
        })?;
    NumericTensorBack::prepare(projection, reserved)
        .map_err(|error| format!("Tensor prepared resource admission: {error:?}"))
}
