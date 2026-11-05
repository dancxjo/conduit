//! Check a finite selection before lending disjoint retained library entries.
pub(crate) fn append(code: &mut String, entries: usize) {
    code.push_str(&format!(r#"
/// Check every selected assignment before lending any retained backing.
/// None leaves an entry untouched. Returned loans coexist without moving Backs.
/// Root still owns complete-workset admission, authority and retirement.
pub fn borrow_selected(&mut self,mut selected:[Option<conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>>;{entries}])->Result<(
"#));
    for index in 0..entries {
        code.push_str(&format!("Option<(u16,entry_{index}::BorrowedBack<'_>)>,\n"));
    }
    code.push_str(
        "),conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{\n",
    );
    // No availability changes until every exact identity/bound check succeeds.
    for index in 0..entries {
        code.push_str(&format!(r#"
let node_{index}=selected[{index}].take().map(|selection|{{
self.entry_{index}.as_ref().ok_or(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner)?.check_assigned(selection)
}}).transpose()?;
"#));
    }
    code.push_str("Ok((\n");
    // Each mutable loan names a distinct field. All fallible checking is over;
    // returning the loans does not decode/validate the assignments a second time.
    for index in 0..entries {
        code.push_str(&format!(r#"
node_{index}.map(|node|(node,self.entry_{index}.as_mut().expect("preflighted resident owner").borrow_prechecked())),
"#));
    }
    code.push_str("))}\n");
    code.push_str(&format!("/// The same preflighted loans in a finite uniform array; entry identities remain nominal.\npub fn borrow_selected_array(&mut self,selected:[Option<conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>>;{entries}])->Result<[Option<(u16,BorrowedBack<'_>)>;{entries}],conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{\nlet ("));
    for index in 0..entries {
        code.push_str(&format!("loan_{index},"));
    }
    code.push_str(")=self.borrow_selected(selected)?;\nOk([\n");
    for index in 0..entries {
        code.push_str(&format!(
            "loan_{index}.map(|(node,back)|(node,BorrowedBack::Entry{index}(back))),\n"
        ));
    }
    code.push_str("]) }\n");
}

/// A representation adapter over the existing nominal loans, not another Back.
pub(crate) fn append_back(code: &mut String, entries: usize) {
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
    for (signature, body) in [
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
