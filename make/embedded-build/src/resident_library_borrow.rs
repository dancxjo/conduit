//! Check a finite selection before lending disjoint retained library entries.
#[cfg(test)]
#[path = "resident_assignment_selection_tests.rs"]
mod selection_tests;
pub(crate) fn append(code: &mut String, entries: usize) {
    code.push_str(&format!(r#"
/// Check every assignment while retaining an exclusive, non-executing loan.
/// The ticket is not a resource reservation or execution authorization.
#[inline(always)]
pub fn prepare_selected_array(&mut self,selected:[Option<conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>>;{entries}])->Result<PreparedResidentLoans<'_>,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{
 let mut selected=selected.into_iter();
 self.prepare_selected_profiles_by(|_|Ok(selected.next().flatten().map(ResidentAssignment::Local)))
}}
/// Check the complete mixed representation before lending any owner. Profiles
/// are cold preparation inputs; the returned ticket retains only node ordinals.
#[inline(always)]
pub fn prepare_selected_profiles_array(&mut self,mut selected:[Option<ResidentAssignment<'_>>;{entries}])->Result<PreparedResidentLoans<'_>,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{
 self.prepare_selected_profiles_by(|index|Ok(selected[index].take()))
}}
/// Inspect exactly one assignment at a time in entry order. The accessor's
/// original binding error propagates unchanged. Earlier checks lend no owner;
/// a late refusal leaves the complete original availability unchanged.
/// Only compact checked node ordinals survive successful preflight.
#[inline(always)]
pub fn prepare_selected_profiles_by<'s>(&mut self,mut selected:impl FnMut(usize)->Result<Option<ResidentAssignment<'s>>,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>)->Result<PreparedResidentLoans<'_>,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{
"#));
    code.push_str(&format!("let mut nodes=[None;{entries}];\nfor (index,node) in nodes.iter_mut().enumerate(){{\nif let Some(selection)=selected(index)?{{\n*node=Some(match index{{\n"));
    for index in 0..entries {
        code.push_str(&format!(r#"
{index}=>{{
let owner=self.entry_{index}.as_ref().ok_or(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner)?;
match selection{{ResidentAssignment::Local(plan)=>owner.check_assigned(plan),ResidentAssignment::LineFore(plan)=>owner.check_assigned_with_line_fore(plan)}}
}},
"#));
    }
    code.push_str("_=>unreachable!(\"bounded resident entry index\"),} ?);\n}\n}\nOk(PreparedResidentLoans{owner:self,nodes})}\n");
    code.push_str(&format!("/// Check the entire selection before lending any backing.\npub fn borrow_selected(&mut self,selected:[Option<conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>>;{entries}])->Result<(\n"));
    for index in 0..entries {
        code.push_str(&format!("Option<(u16,entry_{index}::BorrowedBack<'_>)>,\n"));
    }
    code.push_str("),conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{Ok(self.prepare_selected_array(selected)?.borrow_tuple())}\n");
    code.push_str(&format!("/// The same exact loans in a finite uniform array.\npub fn borrow_selected_array(&mut self,selected:[Option<conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>>;{entries}])->Result<[Option<(u16,BorrowedBack<'_>)>;{entries}],conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{Ok(self.prepare_selected_array(selected)?.bind())}}\n"));
}

fn append_prepared(code: &mut String, entries: usize, workspace: bool) {
    code.push_str(&format!(
        r#"
/// Exact checked selection over exclusively held initialized owners. Dropping
/// before binding changes no availability. Root still admits resources and
/// authority independently; this ticket cannot Start a kernel.
pub struct PreparedResidentLoans<'a>{{owner:&'a mut Initialized,nodes:[Option<u16>;{entries}]}}
impl<'a> PreparedResidentLoans<'a>{{
/// Exact preflighted node mapping; no ownership, allocation or authority transfer.
pub fn nodes(&self)->[Option<u16>;{entries}]{{self.nodes}}
/// Infallible, allocation-free binding after Root's complete admission.
pub fn bind(self)->[Option<(u16,BorrowedBack<'a>)>;{entries}]{{let (
"#
    ));
    for index in 0..entries {
        code.push_str(&format!("loan_{index},"));
    }
    code.push_str(")=self.borrow_tuple();[\n");
    for index in 0..entries {
        code.push_str(&format!(
            "loan_{index}.map(|(node,back)|(node,BorrowedBack::Entry{index}(back))),\n"
        ));
    }
    code.push_str("]}\npub fn terminal_transductions<const P:usize>(&self)->[Option<[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>;P]>;");
    code.push_str(&entries.to_string());
    code.push_str("]{[\n");
    for index in 0..entries {
        code.push_str(&format!("self.nodes[{index}].map(|_|self.owner.entry_{index}.as_ref().expect(\"exclusively preflighted resident owner\").terminal_transductions::<P>()),\n"));
    }
    code.push_str("]}\n");
    if workspace {
        code.push_str("pub fn execution_workspace_requirements_at<const P:usize>(&self,slot:usize)->Option<Option<conduit_kernel::scheduler::StepWorkspaceRequirements>>{match slot{\n");
        for i in 0..entries {
            code.push_str(&format!("{i}=>Some(if self.nodes[{i}].is_some(){{self.owner.entry_{i}.as_ref()?.execution_workspace_requirements::<P>()}}else{{None}}),\n"));
        }
        code.push_str("_=>None,}}\n");
    }
    code.push_str("pub fn split(self)->[Option<PreparedResidentEntryLoan<'a>>;");
    code.push_str(&entries.to_string());
    code.push_str("]{let Self{owner,nodes}=self;[\n");
    for index in 0..entries {
        code.push_str(&format!("nodes[{index}].map(|node|PreparedResidentEntryLoan{{inner:PreparedResidentEntryInner::Entry{index}{{node,owner:owner.entry_{index}.as_mut().expect(\"exclusively preflighted resident owner\")}}}}),\n"));
    }
    code.push_str("]}\nfn borrow_tuple(self)->(\n");
    for index in 0..entries {
        code.push_str(&format!("Option<(u16,entry_{index}::BorrowedBack<'a>)>,\n"));
    }
    code.push_str("){let Self{owner,nodes}=self;(\n");
    for index in 0..entries {
        code.push_str(&format!("nodes[{index}].map(|node|(node,owner.entry_{index}.as_mut().expect(\"exclusively preflighted resident owner\").borrow_prechecked())),\n"));
    }
    code.push_str(")} }\n");
    code.push_str("/// Disjoint, unstarted entry loans. Splitting changes no availability.\npub struct PreparedResidentEntryLoan<'a>{inner:PreparedResidentEntryInner<'a>}\nenum PreparedResidentEntryInner<'a>{\n");
    for index in 0..entries {
        code.push_str(&format!(
            "Entry{index}{{node:u16,owner:&'a mut entry_{index}::Initialized}},\n"
        ));
    }
    code.push_str(
        "}\nimpl<'a> PreparedResidentEntryLoan<'a>{pub fn node(&self)->u16{match &self.inner{\n",
    );
    for index in 0..entries {
        code.push_str(&format!(
            "PreparedResidentEntryInner::Entry{index}{{node,..}}=>*node,\n"
        ));
    }
    code.push_str("}}\npub fn terminal_transductions<const P:usize>(&self)->[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>;P]{match &self.inner{\n");
    for index in 0..entries {
        code.push_str(&format!("PreparedResidentEntryInner::Entry{index}{{owner,..}}=>owner.terminal_transductions::<P>(),\n"));
    }
    code.push_str("}}\n");
    if workspace {
        code.push_str("pub fn execution_workspace_requirements<const P:usize>(&self)->Option<conduit_kernel::scheduler::StepWorkspaceRequirements>{match &self.inner{\n");
        for i in 0..entries {
            code.push_str(&format!("PreparedResidentEntryInner::Entry{i}{{owner,..}}=>owner.execution_workspace_requirements::<P>(),\n"));
        }
        code.push_str("}}\n");
    }
    code.push_str("pub fn bind(self)->(u16,BorrowedBack<'a>){match self.inner{\n");
    for index in 0..entries {
        code.push_str(&format!("PreparedResidentEntryInner::Entry{index}{{node,owner}}=>(node,BorrowedBack::Entry{index}(owner.borrow_prechecked())),\n"));
    }
    code.push_str("}}}\n");
}

/// A representation adapter over the existing nominal loans, not another Back.
pub(crate) fn append_back(code: &mut String, entries: usize, workspace: bool) {
    code.push_str(include_str!("resident_assignment_source.rs"));
    append_prepared(code, entries, workspace);
    code.push_str("/// Uniform selected loans; each variant still owns its exact entry's exclusive loan.\npub enum BorrowedBack<'a>{\n");
    for index in 0..entries {
        code.push_str(&format!("Entry{index}(entry_{index}::BorrowedBack<'a>),\n"));
    }
    code.push_str("}\nimpl BorrowedBack<'_>{\n/// Only explicit successful retirement republishes the original entry.\npub fn finish_retired(self)->Result<(),Self>{match self{\n");
    for index in 0..entries {
        code.push_str(&format!(
            "Self::Entry{index}(back)=>back.finish_retired().map_err(Self::Entry{index}),\n"
        ));
    }
    code.push_str(
        "}}}\nimpl<const P:usize>conduit_kernel::scheduler::StepBack<P> for BorrowedBack<'_>{\n",
    );
    if workspace {
        for (signature, body) in [
            ("fn workspace_requirements(&self)->Option<conduit_kernel::scheduler::StepWorkspaceRequirements>", "conduit_kernel::scheduler::StepBack::<P>::workspace_requirements(back)"),
            ("fn step_with_workspace(&mut self,io:&mut conduit_kernel::scheduler::StepIo<P>,input:&conduit_kernel::scheduler::StepInputBytes<'_,P>,workspace:&mut conduit_kernel::scheduler::StepWorkspace<'_>)->conduit_kernel::scheduler::StepOutcome", "conduit_kernel::scheduler::StepBack::step_with_workspace(back,io,input,workspace)"),
        ] {
            code.push_str(signature); code.push_str("{match self{\n");
            for index in 0..entries { code.push_str(&format!("Self::Entry{index}(back)=>{body},\n")); }
            code.push_str("}}\n");
        }
    }
    for (signature, body) in [
        ("fn canonical_input_schema(&self,port:conduit_kernel::PortId)->Option<&[u8]>", "conduit_kernel::scheduler::StepBack::<P>::canonical_input_schema(back,port)"),
        ("fn canonical_output_schema(&self,port:conduit_kernel::PortId)->Option<&[u8]>", "conduit_kernel::scheduler::StepBack::<P>::canonical_output_schema(back,port)"),
        ("fn terminal_transductions(&self)->[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>;P]", "conduit_kernel::scheduler::StepBack::<P>::terminal_transductions(back)"),
        ("fn step(&mut self,io:&mut conduit_kernel::scheduler::StepIo<P>,input:&conduit_kernel::scheduler::StepInputBytes<'_,P>)->conduit_kernel::scheduler::StepOutcome", "conduit_kernel::scheduler::StepBack::step(back,io,input)"),
        ("fn prepared_output(&self,port:conduit_kernel::PortId)->Option<&[u8]>", "conduit_kernel::scheduler::StepBack::<P>::prepared_output(back,port)"),
        ("fn step_committed(&mut self)", "conduit_kernel::scheduler::StepBack::<P>::step_committed(back)"),
        ("fn cancel(&mut self)", "conduit_kernel::scheduler::StepBack::<P>::cancel(back)"),
    ] {
        code.push_str(signature);
        code.push_str("{match self{\n");
        for index in 0..entries {
            code.push_str(&format!("Self::Entry{index}(back)=>{body},\n"));
        }
        code.push_str("}}\n");
    }
    code.push_str("}\n");
}
