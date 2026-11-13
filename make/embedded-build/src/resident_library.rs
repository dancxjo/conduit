//! Compose immutable images without publishing a partially initialized inventory.
use crate::{PreparedResidentExpression, ResidentExpressionRefusal as R};

/// Emit one owner for an explicitly selected finite collection of resident Backs.
/// This admits neither their storage nor a Host, work pool, reservation or Play.
pub fn generate_resident_library(
    images: &[PreparedResidentExpression],
    maximum_generated_bytes: u32,
) -> Result<String, R> {
    if images.len() > 64 {
        return Err(R::Bounds);
    }
    if images.is_empty() {
        let code = concat!(
            include_str!("resident_assignment_source.rs"),
            include_str!("resident_library_empty_source.rs")
        );
        return if code.len() as u64 <= u64::from(maximum_generated_bytes) {
            Ok(code.into())
        } else {
            Err(R::Bounds)
        };
    }
    let mut code = format!("pub const ENTRY_COUNT:usize={};\n", images.len());
    let maximum_value_bytes = images
        .iter()
        .map(|image| {
            image
                .maximum_input_bytes()
                .max(image.maximum_output_bytes())
        })
        .max()
        .ok_or(R::Bounds)?;
    code.push_str(&format!("/// Canonical extent of the selected prepared programs; not a queue grant.\npub const VALUE_SLOT_BYTES:usize={maximum_value_bytes};\n"));
    for (index, image) in images.iter().enumerate() {
        if images[..index].iter().any(|prior| {
            prior.offer_description().capability_id == image.offer_description().capability_id
        }) {
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
        "/// Lossless document images of unbound initialized owners only.\n/// This does not publish a Host generation.\npub fn offers(&self)->[Option<&'static [u8]>;{}]{{[\n",
        images.len()
    ));
    for index in 0..images.len() {
        code.push_str(&format!(
            "self.entry_{index}.as_ref().and_then(|owner|owner.available_offer_image()),\n"
        ));
    }
    code.push_str("]}\n");
    code.push_str("pub fn available_identity(&self,index:usize)->Option<&'static conduit_plan_lowering::selected_assigned_expression::ResidentExpressionIdentity>{match index{\n");
    for index in 0..images.len() {
        code.push_str(&format!(
            "{index}=>self.entry_{index}.as_ref().and_then(|owner|owner.available_identity()),\n"
        ));
    }
    code.push_str("_=>None,}}\n");
    code.push_str("/// Actual available entry's prepared canonical envelope, not a grant.\npub fn available_value_slot_bytes(&self,index:usize)->Option<usize>{match index{\n");
    for (index, image) in images.iter().enumerate() {
        let bytes = image
            .maximum_input_bytes()
            .max(image.maximum_output_bytes());
        code.push_str(&format!("{index}=>self.entry_{index}.as_ref().and_then(|owner|owner.available_identity()).map(|_|{bytes}),\n"));
    }
    code.push_str("_=>None,}}\n");
    code.push_str(&format!("pub fn identify_assignments(&self,bytes:&[u8])->Result<conduit_plan_lowering::resident_inventory::ResidentInventorySelection<{}>,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{conduit_plan_lowering::resident_inventory::select_resident_inventory(bytes,|index|self.available_identity(index))}}\n",images.len()));
    code.push_str(&format!("/// Independent local non-resident identities skip computation lookup only. Native binding remains mandatory.\npub fn identify_mixed_assignments(&self,bytes:&[u8],other:&[conduit_plan_lowering::resident_inventory::OtherNodeIdentity])->Result<conduit_plan_lowering::resident_inventory::ResidentInventorySelection<{}>,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{conduit_plan_lowering::resident_inventory::select_resident_inventory_with_other_nodes(bytes,other,|index|self.available_identity(index))}}\n",images.len()));
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
    code.push_str("pub fn check_entry(&self,index:usize,selected:conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>)->Result<u16,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{match index{\n");
    for index in 0..images.len() {
        code.push_str(&format!("{index}=>self.entry_{index}.as_ref().ok_or(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner)?.check_assigned(selected),\n"));
    }
    code.push_str("_=>Err(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::Selection),}}\n");
    crate::resident_library_borrow::append(&mut code, images.len());
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
    crate::resident_library_borrow::append_back(&mut code, images.len());
    if code.len() as u64 > u64::from(maximum_generated_bytes) {
        return Err(R::Bounds);
    }
    Ok(code)
}
