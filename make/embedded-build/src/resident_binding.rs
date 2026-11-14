//! Emit independent immutable selection facts, not an assignment supplied by a peer.
use crate::ResidentExpressionRefusal as R;
use conduit_core as c;

pub(crate) fn append(
    code: &mut String,
    offer: &c::CapabilityOffer,
    text: &str,
    private_work: u16,
) -> Result<(), R> {
    let pin = c::AssignedComputationRequirement::from_text(0, "program", text)
        .map_err(|_| R::GenerationMetadata)?;
    let [input] = offer.inputs.as_slice() else {
        return Err(R::Contract);
    };
    let [output] = offer.outputs.as_slice() else {
        return Err(R::Contract);
    };
    let id = |text: &str| {
        format!(
            "conduit_core::AssignedIdentity({:?})",
            c::AssignedIdentity::from_text(text).0
        )
    };
    let key = format!("conduit_core::AssignedIdentity({:?})", pin.key.0);
    code.push_str(&format!(r#"
static BINDING:conduit_plan_lowering::selected_assigned_expression::ResidentExpressionIdentity=
conduit_plan_lowering::selected_assigned_expression::ResidentExpressionIdentity{{
kind:{kind},implementation:{implementation},artifact:{artifact},
input_port:{input_port},input_kind:{input_kind},output_port:{output_port},output_kind:{output_kind},
computation:conduit_core::AssignedComputationRequirement{{node:0,key:{key},text_bytes:{bytes},text_digest:{digest:?}}},
private_work_units:{private_work},
}};
impl Initialized{{
/// Immutable installed facts remain in flash; availability is checked here.
pub fn available_identity(&self)->Option<&'static conduit_plan_lowering::selected_assigned_expression::ResidentExpressionIdentity>{{self.available.then_some(&BINDING)}}
/// A local inventory pin; the node index still comes from Root's selected assignment.
pub fn computation_requirement(&self,node:u16)->conduit_core::AssignedComputationRequirement{{
conduit_core::AssignedComputationRequirement{{node,..BINDING.computation}}
}}
pub fn check_assigned(&self,selected:conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>)->Result<u16,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{
if !self.available{{return Err(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner);}}
conduit_plan_lowering::selected_assigned_expression::check_selected_resident_expression(BINDING,selected).map(|node|node.0)
}}
/// Explicit Line/Fore preparation checks the same installed owner and computation.
pub fn check_assigned_with_line_fore(&self,selected:conduit_plan_lowering::selected_assigned_expression::SelectedAssignedLineForeExpressionPlan<'_>)->Result<u16,conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{
if !self.available{{return Err(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner);}}
conduit_plan_lowering::selected_assigned_expression::check_selected_resident_expression_with_line_fore(BINDING,selected).map(|node|node.0)
}}
/// All fallible checks precede the one-time transfer; refusal retains the same Back.
pub fn bind_assigned_retained(slot:&mut Option<Self>,selected:conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>)->Result<(u16,Back),conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{{
let node=slot.as_ref().ok_or(conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal::MissingOwner)?.check_assigned(selected)?;
let owner=slot.take().expect("preflighted initialized expression owner");Ok((node,owner.into_back()))
}}
}}
"#,
        kind=id(offer.kind_id.as_str()),
        implementation=id(offer.implementation.implementation_id.as_str()),
        artifact=id(offer.implementation.artifact_id.as_str()),
        input_port=id(input.port_id.as_str()), input_kind=id(input.value_kind.as_str()),
        output_port=id(output.port_id.as_str()), output_kind=id(output.value_kind.as_str()),

        bytes=pin.text_bytes, digest=pin.text_digest,
    ));
    Ok(())
}
