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
}
