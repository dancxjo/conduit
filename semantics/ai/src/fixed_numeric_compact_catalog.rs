//! Exact portable contracts for unit-domain signed-Q7 tiled matrix operations.
//! Source selects this explicit quantization/packing profile and composition.
use crate::fixed_numeric_catalog::{
    FIXED_NUMERIC_SIGNAL_SOURCE, FIXED_NUMERIC_SOURCE, fixed_numeric_type,
};
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_plot::{
    CheckedNativeType, StartupCatalog, check_syntax_document, parse_syntax_document,
};
pub const COMPACT_IMPLEMENTATION: &str = "conduit.numeric/signed-q7-i32-tiled-resource@1";
pub const COMPACT_SOURCE: &str = include_str!("../fixed_numeric_compact.conduit");
pub const COMPACT_SHAPES: &[(usize, usize)] = &[
    (4, 40),
    (192, 128),
    (128, 320),
    (328, 192),
    (192, 192),
    (272, 480),
    (160, 480),
    (240, 384),
    (128, 384),
    (208, 384),
    (160, 160),
    (128, 128),
    (688, 128),
    (128, 40),
];
fn types_uncached() -> Result<Vec<CheckedNativeType>, String> {
    let mut catalog = StartupCatalog::new();
    catalog.insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))?;
    check_syntax_document(
        &parse_syntax_document(&format!(
            "{FIXED_NUMERIC_SOURCE}\n{FIXED_NUMERIC_SIGNAL_SOURCE}\n{COMPACT_SOURCE}"
        )),
        &catalog,
    )
    .map(|doc| doc.native_types)
    .map_err(|error| format!("{}: {}", error.code, error.message))
}
#[cfg(feature = "hosted-catalog-cache")]
fn types() -> Result<Vec<CheckedNativeType>, String> {
    static TYPES: std::sync::OnceLock<Result<Vec<CheckedNativeType>, String>> =
        std::sync::OnceLock::new();
    TYPES.get_or_init(types_uncached).clone()
}
#[cfg(not(feature = "hosted-catalog-cache"))]
fn types() -> Result<Vec<CheckedNativeType>, String> {
    types_uncached()
}
pub fn fixed_compact_type(name: &str) -> Result<StructuredInfoType, String> {
    types()?
        .into_iter()
        .find(|ty| ty.name == name)
        .map(|ty| ty.value_type)
        .ok_or_else(|| format!("unknown compact Type {name}"))
}
pub fn compact_contract(
    input: usize,
    output: usize,
    biased: bool,
    flow: bool,
) -> Result<Kind, String> {
    if !COMPACT_SHAPES.contains(&(input, output)) {
        return Err("unsupported compact shape".into());
    }
    let id = format!(
        "numeric/{}signed-q7-tiled-{}{input}x{output}",
        if flow { "flow-" } else { "" },
        if biased { "affine" } else { "linear" }
    );
    let descriptors = [
        (
            "value",
            fixed_numeric_type(&format!("NumericF32Vector{input}"))?,
            PortDirection::Input,
        ),
        (
            "weights",
            fixed_compact_type(&format!("NumericI8TiledMatrixRef{input}x{output}"))?,
            PortDirection::Input,
        ),
        (
            "scales",
            fixed_compact_type(&format!("NumericF32ScaleRef{output}"))?,
            PortDirection::Input,
        ),
        (
            "result",
            fixed_numeric_type(&format!("NumericF32Vector{output}"))?,
            PortDirection::Output,
        ),
    ];
    let mut descriptors = Vec::from(descriptors);
    if biased {
        descriptors.insert(
            3,
            (
                "bias",
                fixed_numeric_type(&format!("NumericF32BiasRef{output}"))?,
                PortDirection::Input,
            ),
        );
    }
    let mut inputs = vec![];
    let mut outputs = vec![];
    let mut contracts = vec![];
    for (name, ty, direction) in descriptors {
        let value_kind = ty
            .profile()
            .map_err(|e| format!("{e:?}"))?
            .value_kind()
            .clone();
        let port = port_id(name);
        contracts.push(FrontValueContract {
            location: if direction == PortDirection::Input {
                FrontValueLocation::Input(port.clone())
            } else {
                FrontValueLocation::Output(port.clone())
            },
            contract: CheckedValueContract::new(
                value_kind.clone(),
                crate::transport_envelope::maximum_prepared_transport_value_bytes(&ty)
                    .map_err(|e| format!("{e:?}"))?,
                vec![],
            )
            .map_err(|e| format!("{e:?}"))?,
        });
        let descriptor = PortDescriptor {
            port_id: port,
            value_kind,
            direction,
            temporal: if flow && (name == "value" || name == "result") {
                PortTemporal::Flow { closes: true }
            } else {
                PortTemporal::Value
            },
            abnormal_kind: None,
        };
        if direction == PortDirection::Input {
            inputs.push(descriptor)
        } else {
            outputs.push(descriptor)
        }
    }
    Ok(Kind {
        kind_id: kind_id(&id),
        kind_contract_revision: KindIdentity::from(COMPACT_IMPLEMENTATION),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs,
        outputs,
        semantic_laws: vec![KindSemanticLaw::ValueContracts(contracts)],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 16384,
        },
    })
}
pub fn compact_offer(
    input: usize,
    output: usize,
    biased: bool,
    flow: bool,
) -> Result<CapabilityOffer, String> {
    let kind = compact_contract(input, output, biased, flow)?;
    let capability = CapabilityId::from(format!(
        "{COMPACT_IMPLEMENTATION}/{}",
        kind.kind_id.as_str()
    ));
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: capability,
            execution_profile_id: ExecutionProfileId::from(COMPACT_IMPLEMENTATION),
            implementation_id: ImplementationId::from(COMPACT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(COMPACT_IMPLEMENTATION),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub fn install_compact_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for &(input, output) in COMPACT_SHAPES {
        for biased in [false, true] {
            // Bias profiles are independently supported; unbiased matrices never invent one.
            if biased && fixed_numeric_type(&format!("NumericF32BiasRef{output}")).is_err() {
                continue;
            }
            for flow in [false, true] {
                let kind = compact_contract(input, output, biased, flow)?;
                startup.insert(conduit_plot::KindSignature {
                    kind: kind.kind_id.as_str().into(),
                    startup_parameters: vec![],
                })?;
                startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
                profile.insert_kind(kind).map_err(|e| format!("{e:?}"))?;
            }
        }
    }
    Ok(())
}
