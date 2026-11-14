//! Sparse guard custody and actual-count indexing in the flash child image.
use super::*;
#[test]
fn borrowed_child_table_validates_sparse_guards_once_and_keeps_runtime_refusals() {
    let d = checked("type Input = {\n value: U8\n}\ntype Output = {\n left: U8\n right: U8\n}\nplot calculate (\n input: Input...| >> output: Output...|\n) = {left: .value + 1, right: .value + 2}\n");
    let p = program(&d, "calculate");
    let input = record(
        &p.input_type,
        vec![("value", leaf(member(&p.input_type, "value"), 3))],
    );
    let generated =
        generate_borrowed_static_structured_expression_with_work(&d, &p, bounds(), 4096).unwrap();
    let exercise = r#"
static DENY:conduit_core::FixedIntegerContractSpec<'static>=conduit_core::FixedIntegerContractSpec{value_kind:"value/u8",maximum_bytes:0,range:None};
static GUARDED:conduit_plot::StaticProjectedProgram=conduit_plot::StaticProjectedProgram{
 maximum_input_bytes:child_0::PROGRAM.maximum_input_bytes,
 output:child_0::PROGRAM.output,instructions:child_0::PROGRAM.instructions,
 result:child_0::PROGRAM.result,inputs:child_0::PROGRAM.inputs,output_contract:Some(&DENY)};
static CHILD:[conduit_plot::StaticStructuredChildProgram;2]=[
 conduit_plot::StaticStructuredChildProgram::Scalar(&GUARDED),
 conduit_plot::StaticStructuredChildProgram::Scalar(&GUARDED)];
static VALID:conduit_plot::StaticStructuredChildTable=conduit_plot::StaticStructuredChildTable{children:&CHILD,output_guard_slots:&[0,1]};
static DUPLICATE:conduit_plot::StaticStructuredChildTable=conduit_plot::StaticStructuredChildTable{children:&CHILD,output_guard_slots:&[0,0]};
static OUT_OF_RANGE:conduit_plot::StaticStructuredChildTable=conduit_plot::StaticStructuredChildTable{children:&CHILD,output_guard_slots:&[0,2]};
static MISSING:conduit_plot::StaticStructuredChildTable=conduit_plot::StaticStructuredChildTable{children:&CHILD,output_guard_slots:&[]};
static UNEXPECTED:conduit_plot::StaticStructuredChildTable=conduit_plot::StaticStructuredChildTable{children:&CHILDREN,output_guard_slots:&[0,1]};
static BAD_INDEX:conduit_plot::StaticStructuredProgram=conduit_plot::StaticStructuredProgram{instructions:&[conduit_plot::StructuredTapeInstruction::Value(2)],constants:&[],maximum_input_bytes:PROGRAM.maximum_input_bytes,maximum_output_bytes:PROGRAM.maximum_output_bytes};
static BAD_BRANCH:conduit_plot::StaticStructuredProgram=conduit_plot::StaticStructuredProgram{instructions:&[conduit_plot::StructuredTapeInstruction::BranchFalse{value:2,target:1}],constants:&[],maximum_input_bytes:PROGRAM.maximum_input_bytes,maximum_output_bytes:PROGRAM.maximum_output_bytes};
static BAD_PATH:conduit_plot::StaticStructuredChildTable=conduit_plot::StaticStructuredChildTable{children:&[conduit_plot::StaticStructuredChildProgram::InputPath(&[conduit_plot::StaticExpressionPathStep::Field("")]),conduit_plot::StaticStructuredChildProgram::InputPath(&[])],output_guard_slots:&[]};
pub fn adversarial(input:&[u8]) {
 type Image=conduit_plot::BorrowedStaticStructuredExpression<{input_native::CONTRACT_COUNT},{output_native::CONTRACT_COUNT},2>;
 for table in [&DUPLICATE,&OUT_OF_RANGE,&MISSING,&UNEXPECTED] {
  assert!(Image::from_child_programs(input_native::prepare().unwrap(),output_native::prepare().unwrap(),table,&PROGRAM).is_err());
 }
 assert!(conduit_plot::BorrowedStaticStructuredExpression::<{input_native::CONTRACT_COUNT},{output_native::CONTRACT_COUNT},0>::from_child_programs(input_native::prepare().unwrap(),output_native::prepare().unwrap(),&MISSING,&PROGRAM).is_err());
 for program in [&BAD_INDEX,&BAD_BRANCH] {
  assert!(Image::from_child_programs(input_native::prepare().unwrap(),output_native::prepare().unwrap(),&VALID,program).is_err());
 }
 assert!(conduit_plot::BorrowedStaticStructuredExpression::<{input_native::CONTRACT_COUNT},{output_native::CONTRACT_COUNT},0>::from_child_programs(input_native::prepare().unwrap(),output_native::prepare().unwrap(),&BAD_PATH,&PROGRAM).is_err());
 let mut image=Image::from_child_programs(input_native::prepare().unwrap(),output_native::prepare().unwrap(),&VALID,&PROGRAM).unwrap();
 let mut output=[0xa5;4096];let mut scratch=[0;4096];
 assert!(matches!(image.evaluate_into(input,&mut output,&mut scratch),Err(conduit_plot::StaticStructuredRefusal::Child{..})));
 assert_eq!(output,[0xa5;4096]);
 // Only cached guard slots grow RAM. The child count lives in flash metadata.
 assert!(core::mem::size_of::<Expression>() < core::mem::size_of::<conduit_plot::StaticStructuredExpression<{input_native::CONTRACT_COUNT},{output_native::CONTRACT_COUNT},2>>());
 assert_eq!(core::mem::size_of::<Image>()-core::mem::size_of::<Expression>(),2*core::mem::size_of::<Option<conduit_core::BorrowedFixedIntegerContract<'static>>>());
}
"#;
    compiled_rust::verify_with_libraries(
        &format!("mod image{{\n{}\n{exercise}\n}}", generated.source),
        &format!("fn main(){{image::adversarial(&{input:?});}}"),
        "borrowed-child-guard-slots",
        &["conduit_plot", "conduit_core"],
    );
}
