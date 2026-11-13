//! Nominal generated owners lend their existing backing to the one kernel.
pub(crate) fn append(
    code: &mut String,
    output: u32,
    temporal: &str,
    storage: crate::ResidentExecutionStorage,
) {
    let engine = match storage {
        crate::ResidentExecutionStorage::Owned => format!("conduit_semantic_catalog::StaticStructuredExpressionBack<{{input_native::CONTRACT_COUNT}},{{output_native::CONTRACT_COUNT}},{{CHILD_COUNT}},{output}>"),
        crate::ResidentExecutionStorage::SharedWorkspace | crate::ResidentExecutionStorage::SharedWorkspaceSchema => "conduit_semantic_catalog::BorrowedWorkspaceStructuredExpressionBack<{input_native::CONTRACT_COUNT},{output_native::CONTRACT_COUNT},{GUARD_COUNT}>".into(),
    };
    let constructor = if storage == crate::ResidentExecutionStorage::SharedWorkspaceSchema {
        "from_resident_structured_parts"
    } else {
        "from_resident_structured"
    };
    code.push_str(&format!(r#"
#[derive(Debug)]pub enum PreparationRefusal{{Image(conduit_plot::StaticStructuredRefusal),Back(conduit_semantic_catalog::StaticExpressionBackRefusal)}}
type EngineBack={engine};
pub type Back=EngineBack;
pub fn prepare_back()->Result<Back,PreparationRefusal>{{
let image=prepare().map_err(PreparationRefusal::Image)?;
let back=EngineBack::{constructor}(image,{temporal}).map_err(PreparationRefusal::Back)?;
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
    forward(
        code,
        "BorrowedBack<'_>",
        "self.owner.back",
        storage.uses_workspace(),
    );
}

pub(crate) fn forward(code: &mut String, name: &str, engine: &str, workspace: bool) {
    let forwarding = if workspace {
        format!(
            r#"
fn workspace_requirements(&self)->Option<conduit_kernel::scheduler::StepWorkspaceRequirements>{{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::workspace_requirements(&{engine})}}
fn step_with_workspace(&mut self,io:&mut conduit_kernel::scheduler::StepIo<P>,input:&conduit_kernel::scheduler::StepInputBytes<'_,P>,workspace:&mut conduit_kernel::scheduler::StepWorkspace<'_>)->conduit_kernel::scheduler::StepOutcome{{conduit_kernel::scheduler::StepBack::step_with_workspace(&mut {engine},io,input,workspace)}}
"#
        )
    } else {
        String::new()
    };
    code.push_str(&format!(r#"
impl<const P:usize>conduit_kernel::scheduler::StepBack<P> for {name}{{
{forwarding}
fn canonical_input_schema(&self,port:conduit_kernel::PortId)->Option<&[u8]>{{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::canonical_input_schema(&{engine},port)}}
fn canonical_output_schema(&self,port:conduit_kernel::PortId)->Option<&[u8]>{{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::canonical_output_schema(&{engine},port)}}
fn terminal_transductions(&self)->[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>;P]{{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::terminal_transductions(&{engine})}}
fn step(&mut self,io:&mut conduit_kernel::scheduler::StepIo<P>,input:&conduit_kernel::scheduler::StepInputBytes<'_,P>)->conduit_kernel::scheduler::StepOutcome{{conduit_kernel::scheduler::StepBack::step(&mut {engine},io,input)}}
fn prepared_output(&self,port:conduit_kernel::PortId)->Option<&[u8]>{{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::prepared_output(&{engine},port)}}
fn step_committed(&mut self){{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::step_committed(&mut {engine})}}
fn cancel(&mut self){{<EngineBack as conduit_kernel::scheduler::StepBack<P>>::cancel(&mut {engine})}}
}}
"#));
}
