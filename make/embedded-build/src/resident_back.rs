//! Nominal generated owners lend their existing backing to the one kernel.
pub(crate) fn append(code: &mut String, output: u32, temporal: &str) {
    code.push_str(&format!(r#"
#[derive(Debug)]pub enum PreparationRefusal{{Image(conduit_plot::StaticStructuredRefusal),Back(conduit_semantic_catalog::StaticExpressionBackRefusal)}}
type EngineBack=conduit_semantic_catalog::StaticStructuredExpressionBack<{{input_native::CONTRACT_COUNT}},{{output_native::CONTRACT_COUNT}},{{CHILD_COUNT}},{output}>;
pub type Back=EngineBack;
pub fn prepare_back()->Result<Back,PreparationRefusal>{{
let image=prepare().map_err(PreparationRefusal::Image)?;
let back=EngineBack::from_resident_structured(image,{temporal}).map_err(PreparationRefusal::Back)?;
if back.computation_fuel()>PRIVATE_WORK_UNITS{{return Err(PreparationRefusal::Back(conduit_semantic_catalog::StaticExpressionBackRefusal::Work));}}
Ok(back)
}}
/// An exclusive pointer to retained storage, never another Back allocation.
/// Dropping without explicit retirement leaves the inventory unavailable.
pub struct BorrowedBack<'a>{{owner:&'a mut Initialized}}
impl core::ops::Deref for BorrowedBack<'_>{{type Target=Back;fn deref(&self)->&Back{{&self.owner.back}}}}
impl BorrowedBack<'_>{{
/// Root first drains/retires the kernel and retains its evidence. This resets
/// stateless scratch only; it releases no resource or authority reservation.
pub fn finish_retired(self)->Result<(),Self>{{
if !self.owner.back.reset_after_retirement(){{return Err(self);}}
self.owner.available=true;Ok(())
}}
}}
"#));
    forward(code, "BorrowedBack<'_>", "self.owner.back");
}

pub(crate) fn forward(code: &mut String, name: &str, engine: &str) {
    code.push_str(&format!(r#"
impl<const P:usize>conduit_kernel::scheduler::StepBack<P> for {name}{{
fn terminal_transductions(&self)->[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>;P]{{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::terminal_transductions(&{engine})}}
fn step(&mut self,io:&mut conduit_kernel::scheduler::StepIo<P>,input:&conduit_kernel::scheduler::StepInputBytes<'_,P>)->conduit_kernel::scheduler::StepOutcome{{conduit_kernel::scheduler::StepBack::step(&mut {engine},io,input)}}
fn prepared_output(&self,port:conduit_kernel::PortId)->Option<&[u8]>{{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::prepared_output(&{engine},port)}}
fn step_committed(&mut self){{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::step_committed(&mut {engine})}}
fn cancel(&mut self){{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::cancel(&mut {engine})}}
}}
"#));
}
