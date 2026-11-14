//! Ordinary offer metadata lives in the image; initialization retains its Back.
use crate::{
    ResidentExpressionRefusal as R, RESIDENT_EXPRESSION_IMPLEMENTATION, RESIDENT_EXPRESSION_PROFILE,
};
use conduit_core as c;
use conduit_plot::PortableExpressionProgram;

pub(crate) fn append(
    code: &mut String,
    program: &PortableExpressionProgram,
    temporal: c::PortTemporal,
    artifact: &c::ArtifactId,
    private_work: u16,
    instance: Option<&str>,
) -> Result<c::CapabilityOffer, R> {
    let kind = conduit_semantic_catalog::pure_expression_contract(program, temporal)
        .map_err(|_| R::Contract)?;
    let limits = c::CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: program
            .maximum_prepared_input_bytes()
            .map_err(|_| R::Bounds)?
            .max(
                program
                    .maximum_prepared_output_bytes()
                    .map_err(|_| R::Bounds)?,
            )
            .max(1),
    };
    let capability_id = match instance {
        None => format!("{RESIDENT_EXPRESSION_PROFILE}/{}", artifact.as_str()),
        Some(instance) => {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(instance.as_bytes());
            format!(
                "{RESIDENT_EXPRESSION_PROFILE}/{}/instance/{digest:x}",
                artifact.as_str()
            )
        }
    };
    let back = c::Back {
        capability_id: capability_id.into(),
        execution_profile_id: RESIDENT_EXPRESSION_PROFILE.into(),
        implementation_id: RESIDENT_EXPRESSION_IMPLEMENTATION.into(),
        artifact_id: artifact.clone(),
        host_calls: vec![],
        resource_requirements: vec![c::ResourceRequirement {
            class_id: c::COOPERATIVE_STEP_WORK_CLASS_ID.into(),
            units: u32::from(private_work),
            protected_role: None,
            compute: None,
            content: None,
        }],
        authority_requirements: vec![],
    };
    let offer = c::BackOfferBuilder::try_new(kind, back)
        .map_err(|_| R::Contract)?
        .narrow_capacity(limits)
        .map_err(|_| R::Contract)?
        .build();
    let image = crate::generate_capability_offer_image(&offer, usize::MAX)
        .map_err(|_| R::GenerationMetadata)?;
    // Keep only the lossless image, never a second raw catalog in firmware.
    code.push_str(&format!("static OFFER_IMAGE:&[u8]=&{image:?};\n"));
    code.push_str(
        r#"
/// One actually initialized Back; metadata grants no resource or execution.
pub struct Initialized { back:Back, available:bool }
impl Initialized {
pub fn prepare()->Result<Self,PreparationRefusal>{Ok(Self{back:prepare_back()?,available:true})}
/// Lossless descriptive image; availability remains separate.
pub fn offer_image(&self)->&'static [u8]{OFFER_IMAGE}
pub fn available_offer_image(&self)->Option<&'static [u8]>{self.available.then_some(OFFER_IMAGE)}
pub fn into_back(self)->Back{self.back}
"#,
    );
    append_terminal_inspection(code);
    code.push_str(r#"
/// All checks precede the exclusive loan. No second engine is constructed.
pub fn borrow_assigned(&mut self,selected:conduit_plan_lowering::selected_assigned_expression::SelectedAssignedExpressionPlan<'_>)->Result<(u16,BorrowedBack<'_>),conduit_plan_lowering::selected_assigned_expression::ResidentExpressionBindingRefusal>{
let node=self.check_assigned(selected)?;Ok((node,self.borrow_prechecked()))
}
/// Library batch preflight has checked this exact owner under an exclusive borrow.
pub(super) fn borrow_prechecked(&mut self)->BorrowedBack<'_>{self.available=false;BorrowedBack{owner:self}}
}
"#);
    crate::resident_binding::append(
        code,
        &offer,
        &program.canonical_hex().map_err(|_| R::Program)?,
        private_work,
    )?;
    Ok(offer)
}

pub(crate) fn append_terminal_inspection(code: &mut String) {
    code.push_str(r#"
/// Borrow the actual initialized Back's laws without changing availability.
pub fn terminal_transductions<const P:usize>(&self)->[Option<conduit_kernel::scheduler::AssignedTerminalTransduction>;P]{<Back as conduit_kernel::scheduler::StepBack<P>>::terminal_transductions(&self.back)}
"#);
}
