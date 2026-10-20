//! Compile CURRENT generated consumers; admission remains actual Native Source laws.
use conduit_core::*;
use conduit_plot::{rust_binding::*, *};
#[path = "common/generated_rust.rs"]
mod generated_rust;
const SOURCE: &str = r#"
type Label = Text <= 32B
type Interval = {
 end: U64 in 0..=9
 start: U64 in 0..=9
 where .start <= .end
}
type Choice =
 empty
 | interval Interval
 | label Label
type Holder = {
 left: Choice
 right: Choice?
}
type Other = {
 value: U64
}
"#;
fn document() -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap()
}
#[test]
fn generated_payload_and_optional_storage_compile_and_preserve_all_admission_laws() {
    let doc = document();
    let options = RustBindingOptions::default();
    let ordinary = generate_rust_bindings(&doc.native_types, &options).unwrap();
    let generated =
        generate_rust_bindings_with_prepared_storage(&doc.native_types, &options, &["Holder"])
            .unwrap();
    assert!(generated.source.starts_with(&ordinary.source));
    assert_eq!(generated.semantic_type_bytes, ordinary.semantic_type_bytes);
    let exercise = format!(
        "\nconst SOURCE:&str={SOURCE:?};\nmod allocation_probe {{ {} }}\n{}",
        include_str!("common/allocation_probe.rs"),
        EXERCISE
    );
    generated_rust::exercise(
        &generated.source,
        &exercise,
        6,
        "prepared-native-decode-variants",
    );
}
#[test]
fn boxed_sequence_foreign_and_forged_optional_shapes_refuse_generation() {
    let doc = document();
    let mut boxed = RustBindingOptions::default();
    boxed
        .boxed_variant_payloads
        .insert("Choice.interval".into());
    assert!(
        generate_rust_bindings_with_prepared_storage(&doc.native_types, &boxed, &["Holder"])
            .is_err()
    );
    let seq = check_syntax_document(
        &parse_syntax_document("type Items = sequence U64 <= 4\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert!(generate_rust_bindings_with_prepared_storage(
        &seq.native_types,
        &RustBindingOptions::default(),
        &["Items"]
    )
    .is_err());
    let holder = doc
        .native_types
        .iter()
        .find(|n| n.name == "Holder")
        .unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = holder.value_type.shape() else {
        panic!()
    };
    let optional = fields
        .iter()
        .find(|f| f.name() == "right")
        .unwrap()
        .value_type();
    let StructuredInfoTypeShape::Variant { cases, .. } = optional.shape() else {
        panic!()
    };
    assert!(StructuredInfoType::variant(
        kind_id("ambiguous/optional"),
        vec![cases[0].clone(), cases[0].clone()]
    )
    .is_err());
    let foreign = StructuredInfoType::variant(kind_id("unowned/optional"), cases.clone()).unwrap();
    let forged = StructuredInfoType::variant(
        kind_id("conduit.conduitese.optional.v1"),
        vec![
            StructuredVariantCase::new(
                "none",
                StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
            )
            .unwrap(),
            cases.iter().find(|c| c.tag() == "some").unwrap().clone(),
        ],
    )
    .unwrap();
    for wrong in [foreign, forged] {
        let mut types = doc.native_types.clone();
        let index = types.iter().position(|n| n.name == "Holder").unwrap();
        types[index].value_type = StructuredInfoType::record(
            holder.identity.clone(),
            vec![
                fields.iter().find(|f| f.name() == "left").unwrap().clone(),
                StructuredFieldType::new("right", wrong).unwrap(),
            ],
        )
        .unwrap();
        assert!(generate_rust_bindings_with_prepared_storage(
            &types,
            &RustBindingOptions::default(),
            &["Holder"]
        )
        .is_err());
    }
}
const EXERCISE: &str = r#"
#[global_allocator]
static ALLOCATOR:allocation_probe::Allocator=allocation_probe::Allocator;
#[cfg(test)] mod acceptance {
use super::*;
use conduit_plot::{*,rust_binding::*};
fn doc()->CheckedSyntaxDocument {check_syntax_document(&parse_syntax_document(SOURCE),&StartupCatalog::new()).unwrap()}
fn bounds()->PreparedDecodeBounds {PreparedDecodeBounds{canonical_bytes:16384,leaf_steps:128,text_bytes:64}}
fn value(index:usize)->Holder {
 let interval=||Choice::Interval(ChoiceInterval{end:7,start:2});
 let label=||Choice::Label(Label::new("exact".into()).unwrap());
 match index {
  0=>Holder::new(Choice::Empty,None).unwrap(),
  1=>Holder::new(label(),Some(interval())).unwrap(),
  2=>Holder::new(interval(),Some(label())).unwrap(),
  _=>Holder::new(Choice::Empty,Some(Choice::Empty)).unwrap(),
 }
}
fn admit<'d>(d:&'d CheckedSyntaxDocument,name:&str,bytes:&[u8])->PreparedNativeAdmission<'d>{
 let id=&d.native_types.iter().find(|n|n.name==name).unwrap().identity;
 let mut a=PreparedNativeAdmission::new(d,id,PreparedNativeBounds{canonical_bytes:16384,scopes:128,invariants:128}).unwrap();
 a.begin(bytes).unwrap();while a.step().unwrap()!=PreparedNativeProgress::Admitted{} a
}
#[test]fn all_alternatives_none_some_retain_inactive_buffers_without_growth(){
 let d=doc();
 for index in 0..4 {
  let expected=value(index);let bytes=expected.clone().encode().unwrap();let a=admit(&d,"Holder",&bytes);let proof=a.admitted().unwrap();
  let mut staging=Holder::prepare_storage(&d,bounds()).unwrap();let capacity=staging.canonical_capacity();
  let ((actual,retained,steps),growth)=allocation_probe::observe(||{
   let mut steps=0;loop {steps+=1;if staging.step(&proof).unwrap()==PreparedDecodeProgress::Complete{break;}}
   let (actual,retained)=match staging.finish_retained(&proof){Ok(v)=>v,Err(_)=>panic!("completion refused")};
   (actual,retained,steps)
  });
  assert_eq!(growth.allocations,0);assert_eq!(growth.reallocations,0);assert_eq!(growth.deallocations,0);
  assert_eq!(actual,expected);assert_eq!(steps,Holder::PREPARED_STORAGE_LEAF_STEPS);assert_eq!(retained.canonical_capacity(),capacity);
  // Retained inactive Strings and gate are deliberately dropped outside the probe.
 }
}
#[test]fn witness_is_exact_type_document_and_full_bytes_not_branch_tag_alone(){
 let d=doc();let foreign=doc();let a=admit(&d,"Holder",&value(1).encode().unwrap());let b=admit(&d,"Holder",&value(2).encode().unwrap());
 let foreign_a=admit(&foreign,"Holder",&value(1).encode().unwrap());let other=admit(&d,"Other",&Other::new(7).unwrap().encode().unwrap());
 let mut staging=Holder::prepare_storage(&d,bounds()).unwrap();staging.step(&a.admitted().unwrap()).unwrap();
 assert!(matches!(staging.step(&b.admitted().unwrap()),Err(PreparedDecodeRefusal::ForeignWitness)));
 assert!(staging.finish_retained(&a.admitted().unwrap()).is_err());
 for proof in [foreign_a.admitted().unwrap(),other.admitted().unwrap()] {
  let mut s=Holder::prepare_storage(&d,bounds()).unwrap();assert!(matches!(s.step(&proof),Err(PreparedDecodeRefusal::ForeignWitness)));
 }
}
#[test]fn weakened_nested_law_metadata_refuses_before_storage_allocation(){
 let mut d=doc();let child=d.native_types.iter_mut().find(|n|n.name=="Interval").unwrap();assert!(!child.invariants.is_empty());child.invariants.clear();
 assert!(matches!(Holder::prepare_storage(&d,bounds()),Err(PreparedDecodeRefusal::Preparation)));
}
#[test]fn pressure_and_cancel_never_yield_native_and_keep_private_staging(){
 let d=doc();let a=admit(&d,"Holder",&value(0).encode().unwrap());let proof=a.admitted().unwrap();
 let mut tiny=bounds();tiny.canonical_bytes=1;let mut staging=Holder::prepare_storage(&d,tiny).unwrap();
 let (failure,growth)=allocation_probe::observe(||staging.step(&proof));assert!(matches!(failure,Err(PreparedDecodeRefusal::Pressure)));assert_eq!(growth.allocations,0);assert_eq!(growth.reallocations,0);assert_eq!(growth.deallocations,0);
 let mut staging=match staging.finish_retained(&proof){Err((PreparedDecodeRefusal::Pressure,s))=>s,_=>panic!("pressure staging lost")};staging.cancel();
 assert!(matches!(staging.step(&proof),Err(PreparedDecodeRefusal::Cancelled)));
}
#[test]fn inactive_alternative_laws_are_not_asserted_for_nonexistent_values(){
 let d=doc();let a=admit(&d,"Holder",&value(0).encode().unwrap());let mut staging=Holder::prepare_storage(&d,bounds()).unwrap();let proof=a.admitted().unwrap();
 loop {if staging.step(&proof).unwrap()==PreparedDecodeProgress::Complete{break;}}
 let (actual,_retained)=match staging.finish_retained(&proof){Ok(v)=>v,Err(_)=>panic!()};assert_eq!(actual,value(0));
}
#[test]fn actual_invalid_law_inside_some_payload_never_produces_admitted_witness(){
 // Raw canonical assembly deliberately bypasses generated Native constructors.
 let d=doc();let good=value(1).into_structured().unwrap();
 let StructuredInfoValueShape::Record(fields)=good.shape() else{panic!()};
 let right=fields.iter().find(|f|f.name()=="right").unwrap().value();
 let integer=|v:u64|primitive_into_structured(conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap(),&v).unwrap();
 let interval=conduit_core::StructuredInfoValue::record(Interval::semantic_type().unwrap(),vec![conduit_core::StructuredFieldValue::new("end",integer(2)).unwrap(),conduit_core::StructuredFieldValue::new("start",integer(7)).unwrap()]).unwrap();
 let choice=conduit_core::StructuredInfoValue::variant(Choice::semantic_type().unwrap(),"interval",interval).unwrap();
 let bad=conduit_core::StructuredInfoValue::variant(right.value_type().clone(),"some",choice).unwrap();
 let bytes=conduit_core::StructuredInfoValue::record(Holder::semantic_type().unwrap(),fields.iter().map(|f|conduit_core::StructuredFieldValue::new(f.name(),if f.name()=="right"{bad.clone()}else{f.value().clone()}).unwrap()).collect()).unwrap().canonical_bytes().unwrap();
 let id=&d.native_types.iter().find(|n|n.name=="Holder").unwrap().identity;let mut a=PreparedNativeAdmission::new(&d,id,PreparedNativeBounds{canonical_bytes:16384,scopes:128,invariants:128}).unwrap();a.begin(&bytes).unwrap();
 let failure=loop{match a.step(){Ok(PreparedNativeProgress::Pending)=>(),Ok(_)=>panic!("invalid nested payload law admitted"),Err(e)=>break e}};
 assert!(matches!(failure,PreparedNativeRefusal::Invariant{..}));assert!(a.admitted().is_err());
}
}
"#;
