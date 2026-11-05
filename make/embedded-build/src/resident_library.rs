//! Compose immutable images without publishing a partially initialized inventory.
use crate::{PreparedResidentExpression, ResidentExpressionRefusal as R};

/// Emit one owner for an explicitly selected finite collection of resident Backs.
/// This admits neither their storage nor a Host, work pool, reservation or Play.
pub fn generate_resident_library(
    images: &[PreparedResidentExpression],
    maximum_generated_bytes: u32,
) -> Result<String, R> {
    if images.is_empty() || images.len() > 64 {
        return Err(R::Bounds);
    }
    let mut code = String::new();
    for (index, image) in images.iter().enumerate() {
        if images[..index]
            .iter()
            .any(|prior| prior.kind_id() == image.kind_id())
        {
            return Err(R::Selection);
        }
        code.push_str(&format!(
            "pub mod entry_{index}{{\n{}\n}}\n",
            image.source()
        ));
        if code.len() as u64 > u64::from(maximum_generated_bytes) {
            return Err(R::Bounds);
        }
    }
    code.push_str("#[derive(Debug)]pub enum PreparationRefusal{\n");
    for index in 0..images.len() {
        code.push_str(&format!(
            "Entry{index}(entry_{index}::PreparationRefusal),\n"
        ));
    }
    code.push_str("}\nimpl PreparationRefusal{pub fn entry(&self)->usize{match self{\n");
    for index in 0..images.len() {
        code.push_str(&format!("Self::Entry{index}(_)=>{index},\n"));
    }
    code.push_str("}}}\n/// Retains every initialized Back. Metadata alone is not admission.\npub struct Initialized{\n");
    for index in 0..images.len() {
        code.push_str(&format!(
            "entry_{index}:Option<entry_{index}::Initialized>,\n"
        ));
    }
    code.push_str(
        "}\nimpl Initialized{\npub fn prepare()->Result<Self,PreparationRefusal>{Ok(Self{\n",
    );
    for index in 0..images.len() {
        code.push_str(&format!("entry_{index}:Some(entry_{index}::Initialized::prepare().map_err(PreparationRefusal::Entry{index})?),\n"));
    }
    code.push_str("})}\n");
    code.push_str(&format!(
        "/// Lossless document images of unbound initialized owners only.\n/// This does not publish a Host generation.\npub fn offers(&self)->[Option<&[u8]>;{}]{{[\n",
        images.len()
    ));
    for index in 0..images.len() {
        code.push_str(&format!(
            "self.entry_{index}.as_ref().and_then(|owner|owner.available_offer_image()),\n"
        ));
    }
    code.push_str("]}\n");
    for index in 0..images.len() {
        code.push_str(&format!(r#"
/// Transfer only this selected owner, after all exact assignment checks succeed.
/// Root remains responsible for authorization, reservations and retirement.
pub fn bind_entry_{index}(&mut self,selected:conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>)->Result<(u16,entry_{index}::Back),conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{
entry_{index}::Initialized::bind_assigned_retained(&mut self.entry_{index},selected)
}}
/// Lend retained backing after exact assignment checking, without moving it.
pub fn borrow_entry_{index}(&mut self,selected:conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>)->Result<(u16,entry_{index}::BorrowedBack<'_>),conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{
self.entry_{index}.as_mut().ok_or(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner)?.borrow_assigned(selected)
}}
pub fn computation_requirement_{index}(&self,node:u16)->Result<conduit_core::AssignedComputationRequirement,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{
self.entry_{index}.as_ref().filter(|owner|owner.available_offer_image().is_some()).map(|owner|owner.computation_requirement(node)).ok_or(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner)
}}
"#));
    }
    code.push_str("pub fn into_backs(self)->(\n");
    for index in 0..images.len() {
        code.push_str(&format!("Option<entry_{index}::Back>,\n"));
    }
    code.push_str("){(\n");
    for index in 0..images.len() {
        code.push_str(&format!(
            "self.entry_{index}.map(|owner|owner.into_back()),\n"
        ));
    }
    code.push_str(")}\n}\n");
    if code.len() as u64 > u64::from(maximum_generated_bytes) {
        return Err(R::Bounds);
    }
    Ok(code)
}
