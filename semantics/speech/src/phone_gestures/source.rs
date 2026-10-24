//! Retained checked Source execution; no numerical policy in Rust.
use super::*;
use conduit_plot::PortableExpressionProgram;
const ADVANCE: &str = include_str!(concat!(env!("OUT_DIR"), "/phone_gesture_advance.hex"));
const PHASES: &str = include_str!(concat!(env!("OUT_DIR"), "/phone_gesture_phases.hex"));
const CURVE: &str = include_str!(concat!(env!("OUT_DIR"), "/phone_gesture_curve.hex"));
const ENERGY: &str = include_str!(concat!(env!("OUT_DIR"), "/phone_gesture_energy.hex"));
pub const SOURCE_ID: &str = include_str!(concat!(env!("OUT_DIR"), "/phone_gesture_source_id.txt"));
pub struct GestureSourceExecution {
    program: &'static str,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl GestureSourceExecution {
    pub fn source_id(&self) -> &'static str {
        SOURCE_ID
    }
    pub fn program_hex(&self) -> &'static str {
        self.program
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
}
pub(super) enum Program {
    Advance,
    Phases,
    Curve,
    Energy,
}
pub(super) fn execute<I: NativeRustBinding, O: NativeRustBinding>(
    program: Program,
    input: I,
    receipts: &mut Vec<GestureSourceExecution>,
) -> Result<O, PhoneGestureRefusal> {
    let program = match program {
        Program::Advance => ADVANCE,
        Program::Phases => PHASES,
        Program::Curve => CURVE,
        Program::Energy => ENERGY,
    };
    let input = input.encode().map_err(PhoneGestureRefusal::Native)?;
    if input.len() > 65536 {
        return Err(PhoneGestureRefusal::Capacity);
    }
    let output = PortableExpressionProgram::from_canonical_hex(program)
        .map_err(|_| PhoneGestureRefusal::Source)?
        .evaluate(&input)
        .map_err(PhoneGestureRefusal::Evaluation)?;
    if output.len() > 65536 {
        return Err(PhoneGestureRefusal::Capacity);
    }
    let admitted = O::decode(&output).map_err(PhoneGestureRefusal::Native)?;
    receipts.push(GestureSourceExecution {
        program,
        input,
        output,
    });
    Ok(admitted)
}
pub(super) fn phases(
    timing: SpeechPhoneGestureTiming,
    receipts: &mut Vec<GestureSourceExecution>,
) -> Result<SpeechPhoneGesturePhases, PhoneGestureRefusal> {
    let raw: SpeechPhoneGestureRawPhases = execute(Program::Phases, timing, receipts)?;
    let t = |n| {
        conduit_audio::AudioExactTimeOffset::new(*raw.denominator(), n)
            .map_err(PhoneGestureRefusal::Native)
    };
    SpeechPhoneGesturePhases::new(
        t(*raw.closure_end())?,
        t(*raw.leading_end())?,
        t(*raw.leading_start())?,
        t(*raw.nominal_end())?,
        t(*raw.nominal_start())?,
        t(*raw.release_end())?,
        t(*raw.trailing_end())?,
        t(*raw.trailing_start())?,
    )
    .map_err(PhoneGestureRefusal::Native)
}
