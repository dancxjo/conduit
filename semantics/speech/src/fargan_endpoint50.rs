//! Opaque checked endpoint-profile admission, not an installed neural Back.
//! Borrows the full original frozen projection and all epochs without filtering.
use crate::{fargan_carrier_epochs::PreparedCarrierEpochs, semantic::SpeechFundamentalCycle};
use alloc::vec::Vec;
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PortableExpressionProgram,
};
pub mod native {
    include!(concat!(env!("OUT_DIR"), "/fargan_endpoint50_types.rs"));
}
pub use native::SpeechFarganEndpointRawStep;
use native::{SpeechFarganEndpointCycles50, SpeechFarganEndpointStepInput};
const GATE: &str = include_str!(concat!(env!("OUT_DIR"), "/fargan_endpoint50_gate.hex"));
const STEP: &str = include_str!(concat!(env!("OUT_DIR"), "/fargan_endpoint50_step.hex"));
const SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/fargan_endpoint50_source_id.txt"));
#[derive(Debug)]
pub enum Endpoint50Refusal {
    Count,
    Geometry,
    NotReady { epoch: usize },
    ConstantCycle,
    Native(NativeBindingRefusal),
    Program,
    Execution,
}
pub struct Endpoint50StepReceipt {
    descriptor: SpeechFarganEndpointRawStep,
    input: Vec<u8>,
    output: Vec<u8>,
}
impl Endpoint50StepReceipt {
    pub fn descriptor(&self) -> &SpeechFarganEndpointRawStep {
        &self.descriptor
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn output_canonical(&self) -> &[u8] {
        &self.output
    }
}
/// Immutable borrow preserves complete source identity, projection, exact Audio
/// anchor/basis/grid/control receipts, blockers, periods and original ratios.
/// This admits only the endpoint policy, never installed state/output atomicity.
pub struct PreparedFarganEndpoint50<'e, 'f> {
    epochs: &'e PreparedCarrierEpochs<'f>,
    cycles: Vec<SpeechFundamentalCycle>,
    gate_input: Vec<u8>,
    gate_output: Vec<u8>,
    steps: Vec<Endpoint50StepReceipt>,
}
impl<'e, 'f> PreparedFarganEndpoint50<'e, 'f> {
    pub fn epochs(&self) -> &'e PreparedCarrierEpochs<'f> {
        self.epochs
    }
    pub fn cycles(&self) -> &[SpeechFundamentalCycle] {
        &self.cycles
    }
    pub fn steps(&self) -> &[Endpoint50StepReceipt] {
        &self.steps
    }
    pub fn gate_input_canonical(&self) -> &[u8] {
        &self.gate_input
    }
    pub fn gate_output_canonical(&self) -> &[u8] {
        &self.gate_output
    }
    pub fn source_program_hex(&self) -> &'static str {
        GATE
    }
    pub fn step_program_hex(&self) -> &'static str {
        STEP
    }
    pub fn source_document_id(&self) -> &'static str {
        SOURCE
    }
}
pub fn prepare_fargan_endpoint50<'e, 'f>(
    epochs: &'e PreparedCarrierEpochs<'f>,
) -> Result<PreparedFarganEndpoint50<'e, 'f>, Endpoint50Refusal> {
    use Endpoint50Refusal::*;
    if epochs.epochs().len() != 50 {
        return Err(Count);
    }
    let mut cycles = Vec::with_capacity(50);
    let mut expected_start = 0u64;
    for (index, epoch) in epochs.epochs().iter().enumerate() {
        if !epoch.blockers().is_empty() {
            return Err(NotReady { epoch: index });
        }
        if epoch.start_frame() != expected_start || epoch.pcm().len() != 80 {
            return Err(Geometry);
        }
        // Traversal of retained Audio geometry only; no new clock/control policy.
        expected_start = expected_start
            .checked_add(epoch.pcm().len() as u64)
            .ok_or(Geometry)?;
        let period = epoch.period().ok_or(NotReady { epoch: index })?;
        cycles.push(period.projection().input().cycle().clone());
    }
    if expected_start != epochs.source().total_frames() {
        return Err(Geometry);
    }
    let cycle_array: [SpeechFundamentalCycle; 50] = cycles.clone().try_into().map_err(|_| Count)?;
    let gate_input = SpeechFarganEndpointCycles50::new(cycle_array)
        .map_err(Native)?
        .encode()
        .map_err(Native)?;
    let gate = PortableExpressionProgram::from_canonical_hex(GATE).map_err(|_| Program)?;
    let gate_output = gate.evaluate(&gate_input).map_err(|_| Execution)?;
    // Boolean primitive ABI is one canonical byte. Any other output refuses;
    // no supplied Boolean can create this receipt.
    if gate_output.as_slice() != [1] {
        return Err(ConstantCycle);
    }
    let step = PortableExpressionProgram::from_canonical_hex(STEP).map_err(|_| Program)?;
    let mut steps = Vec::with_capacity(51);
    for index in 0..=50u64 {
        let input = SpeechFarganEndpointStepInput::new(index)
            .map_err(Native)?
            .encode()
            .map_err(Native)?;
        let output = step.evaluate(&input).map_err(|_| Execution)?;
        let descriptor = SpeechFarganEndpointRawStep::decode(&output).map_err(Native)?;
        steps.push(Endpoint50StepReceipt {
            descriptor,
            input,
            output,
        });
    }
    Ok(PreparedFarganEndpoint50 {
        epochs,
        cycles,
        gate_input,
        gate_output,
        steps,
    })
}
