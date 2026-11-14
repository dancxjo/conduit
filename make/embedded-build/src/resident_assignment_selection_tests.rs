//! Generated accessor preflight preserves exact checks and atomic owner custody.
use crate::resident_expression_tests::{compiled_rust, context, plan};
use crate::*;
use conduit_core::{AssignedPlanMaxima, PortTemporal};

#[test]
fn accessor_checks_one_assignment_at_a_time_without_withdrawing_on_late_refusal() {
    let (document, program) = context();
    let bounds = ResidentExpressionBounds {
        native: conduit_plot::rust_binding::PreparedNativeBounds {
            canonical_bytes: 4096,
            scopes: 64,
            invariants: 64,
            contracts: 64,
        },
        maximum_occurrences: 4096,
        maximum_program_bytes: 512 * 1024,
        maximum_generated_bytes: 1024 * 1024,
        private_work_units: 1000,
    };
    let images: Vec<_> = ["left", "right"]
        .into_iter()
        .map(|label| {
            PreparedResidentExpression::prepare_instance(
                &document,
                &program,
                PortTemporal::Value,
                bounds,
                Some(label),
            )
            .unwrap()
        })
        .collect();
    let code = generate_resident_library(&images, 4 * 1024 * 1024).unwrap();
    let mut exercise = String::from(
        r#"
use conduit_core::*;
use conduit_plan_lowering::{assigned_play_scope::AssignedPlayScope,selected_assigned_operation::AssignedOperationInput,selected_assigned_expression::{SelectedAssignedExpressionPlan,ResidentExpressionBindingRefusal as E}};
"#,
    );
    for (index, image) in images.iter().enumerate() {
        let plan = plan(&document, &program, image, 1000);
        let fragment = &plan.fragments[0];
        let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
            &plan,
            &fragment.fragment_id,
        )
        .unwrap();
        let generated = generate_embedded_plan_with_computations(
            fragment,
            &lowered,
            EmbeddedImageBounds::HOST_TOOLING,
            EmbeddedComputationBounds {
                maximum_program_bytes: 512 * 1024,
                maximum_total_program_bytes: 1024 * 1024,
            },
        )
        .unwrap();
        let bound = image
            .bind(
                &plan,
                &fragment.fragment_id,
                &fragment.placements[1].placement_id,
            )
            .unwrap();
        let pin = bound.computation_requirement();
        let encoded = encode_assigned_resident_native_plan(
            &generated,
            AssignedPlanMaxima::TINY_HOST,
            &[bound],
        )
        .unwrap();
        exercise.push_str(&format!(r#"
fn active_{index}()->ActivePlayIdentity{{bind_active_play(&PlanId::from({plan:?}),&HostId::from({host:?}),&BootId::from({boot:?}),1)}}
fn selection_{index}(active:&ActivePlayIdentity)->SelectedAssignedExpressionPlan<'_>{{
 const BYTES:&[u8]=&{encoded:?};
 const PINS:&[AssignedComputationRequirement]=&[AssignedComputationRequirement{{node:{node},key:AssignedIdentity({key:?}),text_bytes:{text_bytes},text_digest:{digest:?}}}];
 SelectedAssignedExpressionPlan{{input:AssignedOperationInput::Contiguous(BYTES),maxima:AssignedPlanMaxima::TINY_HOST,
 requirements:AssignedPlanRequirements{{host:AssignedIdentity::from_text({host:?}),boot:AssignedIdentity::from_text({boot:?}),host_calls:&[],resources:&[0],remote_bindings:&[]}},
 computations:AssignedComputationProfile{{maximum_text_bytes:{text_bytes},maximum_total_text_bytes:{text_bytes},requirements:PINS}},fore:None,
 fragment_id:{fragment:?},scope:AssignedPlayScope::Plot(active),node:conduit_kernel::NodeId(1),work_resource:0}}
}}
"#, plan=plan.plan_id.as_str(),host=fragment.host_id.as_str(),boot=fragment.boot_id.as_str(),fragment=fragment.fragment_id.as_str(),node=pin.node,key=pin.key.0,text_bytes=pin.text_bytes,digest=pin.text_digest));
    }
    exercise.push_str(r#"
fn main(){
 let mut owner=Initialized::prepare().unwrap();let a=active_0();let b=active_1();
 let mut visits=Vec::new();
 let result=owner.prepare_selected_profiles_by(|index|{visits.push(index);if index==0{Ok(Some(ResidentAssignment::Local(selection_0(&a))))}else{Err(E::Scope)}});
 assert!(matches!(result,Err(E::Scope)));drop(result);assert_eq!(visits,[0,1]);
 assert_eq!(owner.offers().into_iter().flatten().count(),2);
 for fault in 0..3 {
  let result=owner.prepare_selected_profiles_by(|index|{
   let mut selected=if index==0||fault==0{selection_0(&a)}else{selection_1(&b)};
   if index==1 {if fault==1{selected.fragment_id="wrong";}if fault==2{selected.work_resource=1;}}
   Ok(Some(ResidentAssignment::Local(selected)))
  });assert!(result.is_err());drop(result);assert_eq!(owner.offers().into_iter().flatten().count(),2);
 }
 let ticket=owner.prepare_selected_profiles_by(|index|Ok(Some(ResidentAssignment::Local(if index==0{selection_0(&a)}else{selection_1(&b)})))).unwrap();
 assert_eq!(ticket.nodes(),[Some(1),Some(1)]);drop(ticket.split());
 assert_eq!(owner.offers().into_iter().flatten().count(),2);
 let ticket=owner.prepare_selected_array([Some(selection_0(&a)),Some(selection_1(&b))]).unwrap();
 assert_eq!(ticket.nodes(),[Some(1),Some(1)]);drop(ticket);
 let result=owner.prepare_selected_profiles_array([Some(ResidentAssignment::Local(selection_0(&a))),Some(ResidentAssignment::Local(selection_0(&a)))]);
 assert!(result.is_err());drop(result);assert_eq!(owner.offers().into_iter().flatten().count(),2);
 let loans=owner.prepare_selected_profiles_by(|index|Ok(if index==0{Some(ResidentAssignment::Local(selection_0(&a)))}else{None})).unwrap().bind();
 assert!(loans[0].is_some()&&loans[1].is_none());drop(loans);
 assert_eq!(owner.offers().into_iter().flatten().count(),1);
}
"#);
    compiled_rust::verify_with_libraries(
        &code,
        &exercise,
        "resident-assignment-accessor",
        &[
            "conduit_kernel",
            "conduit_plot",
            "conduit_core",
            "conduit_semantic_catalog",
            "conduit_plan_lowering",
        ],
    );
}

#[test]
fn empty_inventory_never_calls_assignment_accessor() {
    let code = generate_resident_library(&[], 1024 * 1024).unwrap();
    compiled_rust::verify_with_libraries(
        &code,
        "fn main(){let mut owner=Initialized::prepare().unwrap();let ticket=owner.prepare_selected_profiles_by(|_|panic!(\"empty accessor called\")).unwrap();assert_eq!(ticket.nodes(),[]);drop(ticket);}",
        "empty-resident-assignment-accessor",
        &["conduit_kernel", "conduit_plot", "conduit_core", "conduit_semantic_catalog", "conduit_plan_lowering"],
    );
}
