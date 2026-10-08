//! Exact numeric DSP contracts; no feature layout or waveform policy.
use crate::fixed_numeric_catalog::fixed_numeric_contracts;
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
pub const DSP_IMPLEMENTATION: &str = "conduit.numeric/dsp-f64-libm@1";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedDspOperation {
    RealDft320,
    MagnitudeSquared161,
    Log10_18,
    Log10_1,
    Dot160,
    Sqrt1,
}
impl FixedDspOperation {
    pub fn name(self) -> &'static str {
        match self {
            Self::RealDft320 => "real-dft320",
            Self::MagnitudeSquared161 => "magnitude-squared161",
            Self::Log10_18 => "log10-18",
            Self::Log10_1 => "log10-1",
            Self::Dot160 => "dot160",
            Self::Sqrt1 => "sqrt1",
        }
    }
    pub fn shape(self) -> (usize, usize) {
        match self {
            Self::RealDft320 => (320, 322),
            Self::MagnitudeSquared161 => (322, 161),
            Self::Log10_18 => (18, 18),
            Self::Log10_1 | Self::Sqrt1 => (1, 1),
            Self::Dot160 => (160, 1),
        }
    }
}
pub(crate) fn dsp_specs() -> Vec<crate::fixed_numeric_signal_catalog::Spec> {
    [
        FixedDspOperation::RealDft320,
        FixedDspOperation::MagnitudeSquared161,
        FixedDspOperation::Log10_18,
        FixedDspOperation::Log10_1,
        FixedDspOperation::Dot160,
        FixedDspOperation::Sqrt1,
    ]
    .into_iter()
    .map(|op| {
        let (input, output) = op.shape();
        let mut inputs = vec![("value".into(), format!("NumericF32Vector{input}"))];
        if op == FixedDspOperation::Dot160 {
            inputs = vec![
                ("left".into(), format!("NumericF32Vector{input}")),
                ("right".into(), format!("NumericF32Vector{input}")),
            ];
        }
        (
            format!("numeric/{}", op.name()),
            inputs,
            vec![("result".into(), format!("NumericF32Vector{output}"))],
        )
    })
    .chain([
        (
            String::from("numeric/u16-to-f32"),
            vec![(String::from("value"), String::from("U16"))],
            vec![(String::from("result"), String::from("NumericF32Vector1"))],
        ),
        (
            String::from("numeric/i16-to-f32-160"),
            vec![(String::from("value"), String::from("NumericI16Vector160"))],
            vec![(String::from("result"), String::from("NumericF32Vector160"))],
        ),
    ])
    .chain(core::iter::once((
        String::from("numeric/i16-to-f32-80"),
        vec![(String::from("value"), String::from("NumericI16Vector80"))],
        vec![(String::from("result"), String::from("NumericF32Vector80"))],
    )))
    .chain([1, 18, 20, 160, 320].into_iter().map(|n| {
        (
            format!("numeric/finite-vector{n}"),
            vec![(String::from("value"), format!("NumericRawF32Vector{n}"))],
            vec![(String::from("result"), format!("NumericF32Vector{n}"))],
        )
    }))
    .collect()
}
pub fn fixed_dsp_contract(operation: FixedDspOperation, flow: bool) -> Result<Kind, String> {
    let identity = format!("numeric/{}", operation.name());
    let mut kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == identity)
        .ok_or("absent DSP contract")?;
    if flow {
        kind.kind_id = kind_id(&format!("numeric/flow-{}", operation.name()));
        kind.kind_contract_revision =
            KindIdentity::from(format!("{DSP_IMPLEMENTATION}/closing-flow@1"));
        for p in kind.inputs.iter_mut().chain(&mut kind.outputs) {
            p.temporal = PortTemporal::Flow { closes: true };
        }
    }
    Ok(kind)
}
pub fn fixed_dsp_offer(
    operation: FixedDspOperation,
    flow: bool,
) -> Result<CapabilityOffer, String> {
    let kind = fixed_dsp_contract(operation, flow)?;
    let identity = String::from(kind.kind_id.as_str());
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!("{DSP_IMPLEMENTATION}/{identity}")),
            execution_profile_id: ExecutionProfileId::from(DSP_IMPLEMENTATION),
            implementation_id: ImplementationId::from(DSP_IMPLEMENTATION),
            artifact_id: ArtifactId::from(DSP_IMPLEMENTATION),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}

pub fn install_fixed_dsp_flow_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for operation in [
        FixedDspOperation::RealDft320,
        FixedDspOperation::MagnitudeSquared161,
        FixedDspOperation::Log10_18,
        FixedDspOperation::Log10_1,
        FixedDspOperation::Dot160,
        FixedDspOperation::Sqrt1,
    ] {
        let kind = fixed_dsp_contract(operation, true)?;
        startup.insert(conduit_plot::KindSignature {
            kind: String::from(kind.kind_id.as_str()),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        profile
            .insert_kind(kind)
            .map_err(|error| format!("{error:?}"))?;
    }
    for operation in [
        IntegerConversion::U16,
        IntegerConversion::I16Vector160,
        IntegerConversion::I16Vector80,
        IntegerConversion::FiniteVector(1),
        IntegerConversion::FiniteVector(18),
        IntegerConversion::FiniteVector(20),
        IntegerConversion::FiniteVector(160),
        IntegerConversion::FiniteVector(320),
    ] {
        let kind = fixed_integer_conversion_contract(operation, true)?;
        startup.insert(conduit_plot::KindSignature {
            kind: String::from(kind.kind_id.as_str()),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        profile
            .insert_kind(kind)
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerConversion {
    U16,
    I16Vector160,
    I16Vector80,
    FiniteVector(usize),
}
impl IntegerConversion {
    pub fn name(self) -> String {
        match self {
            Self::U16 => "u16-to-f32".into(),
            Self::I16Vector160 => "i16-to-f32-160".into(),
            Self::I16Vector80 => "i16-to-f32-80".into(),
            Self::FiniteVector(n) => format!("finite-vector{n}"),
        }
    }
    pub fn width(self) -> usize {
        match self {
            Self::U16 => 1,
            Self::I16Vector160 => 160,
            Self::I16Vector80 => 80,
            Self::FiniteVector(n) => n,
        }
    }
}
pub fn fixed_integer_conversion_contract(
    operation: IntegerConversion,
    flow: bool,
) -> Result<Kind, String> {
    let identity = format!("numeric/{}", operation.name());
    let mut kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|kind| kind.kind_id.as_str() == identity)
        .ok_or("absent integer conversion contract")?;
    if flow {
        kind.kind_id = kind_id(&format!("numeric/flow-{}", operation.name()));
        kind.kind_contract_revision =
            KindIdentity::from(format!("{DSP_IMPLEMENTATION}/closing-flow-integer@1"));
        for p in kind.inputs.iter_mut().chain(&mut kind.outputs) {
            p.temporal = PortTemporal::Flow { closes: true };
        }
    }
    Ok(kind)
}
pub fn fixed_integer_conversion_offer(
    operation: IntegerConversion,
    flow: bool,
) -> Result<CapabilityOffer, String> {
    let kind = fixed_integer_conversion_contract(operation, flow)?;
    let identity = String::from(kind.kind_id.as_str());
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: CapabilityId::from(format!("{DSP_IMPLEMENTATION}/{identity}")),
            execution_profile_id: ExecutionProfileId::from(DSP_IMPLEMENTATION),
            implementation_id: ImplementationId::from(DSP_IMPLEMENTATION),
            artifact_id: ArtifactId::from(DSP_IMPLEMENTATION),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
