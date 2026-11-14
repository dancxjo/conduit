//! Compile the checked Source's cold ordinal views as executable and no_std code.
use super::*;
#[test]
fn generated_ordinal_views_keep_exact_remapped_identities_and_clock_refusals() {
    let source = conduit_environment_runtime::construction::compile_default().unwrap();
    let material = compile(&source.expanded, 24 * 1024).unwrap();
    let mapping: Vec<_> = (0..material.node_count).rev().map(|n| n as u16).collect();
    let code = emit_rust_for_layout(&source.expanded, &mapping, 24 * 1024, 1024 * 1024).unwrap();
    let clock = material
        .nodes
        .iter()
        .find(|node| matches!(node.role, SoftwareRole::Tick { .. }))
        .unwrap();
    let SoftwareRole::Tick { count, .. } = clock.role else {
        unreachable!()
    };
    let clock_node = mapping[usize::from(clock.node)];
    let mut exercise = format!(
        r#"
fn main(){{
 use conduit_core::AssignedIdentity as Id;
 use conduit_plan_lowering::resident_inventory::OtherNodeIdentity as Identity;
 use conduit_mcu_host_runtime::execution_factory::SoftwareDefinition as D;
 let wait=conduit_kernel::ValueRef{{slot:7,generation:2,byte_len:8}};
 let clocks=[Identity{{node:{clock_node},kind:Id::from_text("time/tick"),implementation:Id([3;16]),artifact:Id([4;16])}}];
 let waits=[({clock_node},wait)];
 assert!(definition_at({clock_node},&[]).is_err());
 assert!(identity_at({clock_node},&[]).is_err());
 let mut wrong=clocks;wrong[0].kind=Id([0;16]);
 assert!(identity_at({clock_node},&wrong).is_err());
 let definitions=definitions(&waits).unwrap();let identities=identities(&clocks).unwrap();
 for index in 0..NODE_COUNT{{
  match (definition_at(index,&waits).unwrap(),&definitions[index]){{
   (Some(a),Some(b))=>assert_eq!(core::mem::discriminant(&a),core::mem::discriminant(b)),
   (None,None)=>(),_=>panic!("ordinal definition parity"),
  }}
  assert_eq!(identity_at(index,&clocks).unwrap(),identities[index]);
 }}
 let Some(D::Tick{{wait:actual,count}})=definition_at({clock_node},&waits).unwrap() else{{panic!("tick")}};
 assert_eq!(actual,wait);assert_eq!(count,{count});
 assert_eq!(identity_at({clock_node},&clocks).unwrap(),Some(clocks[0]));
 assert!(definition_at(NODE_COUNT,&waits).is_err());assert!(identity_at(NODE_COUNT,&clocks).is_err());
"#
    );
    for node in &material.nodes {
        if let (Some(implementation), Some(artifact)) = (&node.implementation, &node.artifact) {
            writeln!(exercise,"assert_eq!(identity_at({},&clocks).unwrap(),Some(Identity{{node:{},kind:Id({:?}),implementation:Id({:?}),artifact:Id({:?})}}));",mapping[usize::from(node.node)],mapping[usize::from(node.node)],c::AssignedIdentity::from_text(node.kind.as_str()).0,c::AssignedIdentity::from_text(implementation.as_str()).0,c::AssignedIdentity::from_text(artifact.as_str()).0).unwrap();
        }
    }
    exercise.push_str("}\n");
    crate::compiled_rust::verify_with_libraries(
        &code,
        &exercise,
        "source-software-ordinal-views",
        &[
            "conduit_core",
            "conduit_kernel",
            "conduit_plan_lowering",
            "conduit_environment_runtime",
            "conduit_mcu_host_runtime",
        ],
    );
}
