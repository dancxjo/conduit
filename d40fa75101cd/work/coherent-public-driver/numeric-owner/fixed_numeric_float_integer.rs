//! Explicit nearest-ties-away finite F32 to signed16 conversion, without gain/clipping.
use crate::{
    fixed_numeric_catalog::{fixed_numeric_contracts, fixed_numeric_type},
    fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_i16_codec::FixedI16VectorCodec,
    fixed_numeric_preparation::verify_fixed_placement,
};
use alloc::{format, string::String, vec};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub const FLOAT_INTEGER_IMPLEMENTATION: &str = "conduit.numeric/f32-i16-nearest-ties-away160@1";
pub fn float_integer_identity(flow: bool) -> &'static str {
    if flow {
        "numeric/flow-f32-to-i16-nearest-away160"
    } else {
        "numeric/f32-to-i16-nearest-away160"
    }
}
pub fn float_integer_contract(flow: bool) -> Result<Kind, String> {
    let mut kind = fixed_numeric_contracts()?
        .into_iter()
        .find(|k| k.kind_id.as_str() == "numeric/i16-to-f32-160")
        .ok_or("absent exact vector Types")?;
    if kind.inputs.len() != 1 || kind.outputs.len() != 1 {
        return Err("exact conversion ports".into());
    }
    let old_input = kind.inputs[0].value_kind.clone();
    kind.inputs[0].value_kind = kind.outputs[0].value_kind.clone();
    kind.outputs[0].value_kind = old_input;
    let [KindSemanticLaw::ValueContracts(contracts)] = kind.semantic_laws.as_mut_slice() else {
        return Err("exact conversion laws".into());
    };
    if contracts.len() != 2 {
        return Err("exact conversion contracts".into());
    }
    for law in contracts {
        law.location = match &law.location {
            FrontValueLocation::Input(_) => FrontValueLocation::Output(port_id("result")),
            FrontValueLocation::Output(_) => FrontValueLocation::Input(port_id("value")),
            _ => return Err("exact conversion location".into()),
        };
    }
    kind.kind_id = kind_id(float_integer_identity(flow));
    kind.kind_contract_revision = KindIdentity::from(format!(
        "{FLOAT_INTEGER_IMPLEMENTATION}/{}",
        float_integer_identity(flow)
    ));
    let temporal = if flow {
        PortTemporal::Flow { closes: true }
    } else {
        PortTemporal::Value
    };
    kind.inputs[0].temporal = temporal;
    kind.outputs[0].temporal = temporal;
    Ok(kind)
}
pub fn float_integer_offer(flow: bool) -> Result<CapabilityOffer, String> {
    Ok(BackOfferBuilder::new(
        float_integer_contract(flow)?,
        Back {
            capability_id: CapabilityId::from(format!(
                "{FLOAT_INTEGER_IMPLEMENTATION}/{}",
                float_integer_identity(flow)
            )),
            execution_profile_id: FLOAT_INTEGER_IMPLEMENTATION.into(),
            implementation_id: FLOAT_INTEGER_IMPLEMENTATION.into(),
            artifact_id: FLOAT_INTEGER_IMPLEMENTATION.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
pub fn install_float_integer_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profiles: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for flow in [false, true] {
        let kind = float_integer_contract(flow)?;
        startup.insert(conduit_plot::KindSignature {
            kind: kind.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(kind.kind_id.as_str(), kind.checked_front())?;
        profiles.insert_kind(kind).map_err(|e| format!("{e:?}"))?;
    }
    Ok(())
}
/// All values must round to an I16. Refusal leaves the destination unchanged.
pub fn round_f32_to_i16_160(
    input: &[f32; 160],
    output: &mut [i16; 160],
) -> Result<(), FixedFloatIntegerRefusal> {
    let mut candidate = [0; 160];
    for (value, rounded) in input.iter().zip(&mut candidate) {
        if !value.is_finite() {
            return Err(FixedFloatIntegerRefusal::NonFinite);
        }
        let integer = libm::roundf(*value);
        if integer < i16::MIN as f32 || integer > i16::MAX as f32 {
            return Err(FixedFloatIntegerRefusal::Range);
        }
        *rounded = integer as i16;
    }
    *output = candidate;
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixedFloatIntegerRefusal {
    NonFinite,
    Range,
}
pub struct FixedFloatIntegerBack {
    input: FixedF32VectorCodec<160>,
    output: FixedI16VectorCodec<160>,
    flow: bool,
    staged: bool,
    finished: bool,
    cancelled: bool,
    committed_frames: u64,
}
impl FixedFloatIntegerBack {
    pub fn prepare_planned<const PORTS: usize>(
        gear: &PlannedGear,
        fuel: u16,
        flow: bool,
    ) -> Result<Self, String> {
        if PORTS < 1 || fuel < 2 {
            return Err("float integer step bound".into());
        }
        verify_fixed_placement(gear, &float_integer_offer(flow)?).map_err(|e| format!("{e:?}"))?;
        Ok(Self {
            input: FixedF32VectorCodec::prepare(&fixed_numeric_type("NumericF32Vector160")?)
                .map_err(|e| format!("{e:?}"))?,
            output: FixedI16VectorCodec::prepare(&fixed_numeric_type("NumericI16Vector160")?)
                .map_err(|e| format!("{e:?}"))?,
            flow,
            staged: false,
            finished: false,
            cancelled: false,
            committed_frames: 0,
        })
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed_frames
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl<const PORTS: usize> StepBack<PORTS> for FixedFloatIntegerBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2900,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if io.input_closed(PortId(0)) {
            return if self.flow {
                StepOutcome::Complete
            } else {
                fail(2901)
            };
        }
        if self.committed_frames == u64::MAX {
            return fail(2902);
        }
        if io.input(PortId(0)).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(2903);
        };
        let mut value = [0f32; 160];
        let mut result = [0i16; 160];
        if self.input.decode(bytes, &mut value).is_err()
            || round_f32_to_i16_160(&value, &mut result).is_err()
        {
            return fail(2904);
        }
        let length = self.output.encode(&result).len() as u32;
        if io.consume(PortId(0)).is_err() || io.send_prepared(PortId(0), length).is_err() {
            return fail(2905);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then(|| self.output.encoded())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.committed_frames += 1;
            self.finished = !self.flow;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
