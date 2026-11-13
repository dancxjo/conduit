//! Focused compiled resident expression proof.
use super::*;

#[test]
fn parent_checked_children_drop_duplicate_state_only_after_exact_definition_matching() {
    let d = ht();
    let p = program(&d, "ht16k33-frame-request");
    let image = generate_static_structured_expression(&d, &p, bounds(), 4096).unwrap();
    let exercise = r#"
pub fn check_state() {
 let _ = prepare().unwrap();
 assert!(CHILD_COUNT > 0);
 let old_children = core::mem::size_of::<[conduit_plot::StaticStructuredValue<{input_native::CONTRACT_COUNT}>; CHILD_COUNT]>();
 let parents = core::mem::size_of::<conduit_plot::rust_binding::ResidentNativeValidator<{input_native::CONTRACT_COUNT}>>()
  + core::mem::size_of::<conduit_plot::rust_binding::ResidentNativeValidator<{output_native::CONTRACT_COUNT}>>();
 assert!(RUNTIME_STATE_BYTES < old_children + parents);
}
"#;
    compiled_rust::verify(
        &format!("mod image{{\n{image}\n{exercise}\n}}"),
        "fn main(){image::check_state();}",
        "parent-checked-child-state",
    );
    // Supply one child a different prepared definition; the parent stays exact.
    // Equal shape alone cannot justify discarding a child's validator.
    for (name, bound, occurrences) in [("bound", 4095, 4096), ("occurrences", 4096, 4095)] {
        let altered = image.replacen("mod input_native {", &format!("mod input_native {{\npub fn altered()->Result<conduit_plot::rust_binding::ResidentNativeValidator<CONTRACT_COUNT>,conduit_plot::rust_binding::PreparedNativeRefusal>{{conduit_plot::rust_binding::ResidentNativeValidator::new(TYPE,{bound},&CONTRACTS,&SCOPES,{occurrences})}}\n"), 1)
            .replace("Expression::from_child_programs(input,output,&CHILDREN,&PROGRAM)", "let mut children=core::array::from_fn(|_|conduit_plot::StaticStructuredValue::InputPath(&[]));children[0]=conduit_plot::StaticStructuredValue::Scalar(conduit_plot::StaticProjectedExpression::new(input_native::altered().unwrap(),&child_0::PROGRAM).unwrap());Ok(Expression::new(input,output,children,&PROGRAM)?)");
        assert_ne!(altered, image);
        compiled_rust::verify(&format!("mod image{{\n{altered}\n}}"), "fn main(){assert!(matches!(image::prepare(),Err(conduit_plot::StaticStructuredRefusal::Expression(conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram))));}", &format!("parent-child-{name}-mismatch"));
    }
}
