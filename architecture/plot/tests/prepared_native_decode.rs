//! Actual generated staging over existing checked Native-law witnesses.
use conduit_plot::{rust_binding::*, *};
#[path = "common/generated_rust.rs"]
mod generated_rust;
const SOURCE: &str = r#"
type Label = Text <= 32B
type Relation =
    left
    | right
type Interval = {
    end: U64 in 0..=9
    start: U64 in 0..=9
    where .start <= .end
}
type Beam = {
    interval: Interval
    label: Label
    live: Boolean
    relation: Relation
    score: I64 in -16..=16
    slots: collection U64 = 4
}
"#;
fn document() -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap()
}
#[test]
fn generated_closed_storage_materializes_only_actual_admitted_bytes_without_growth() {
    let doc = document();
    let options = RustBindingOptions::default();
    let ordinary = generate_rust_bindings(&doc.native_types, &options).unwrap();
    let generated =
        generate_rust_bindings_with_prepared_storage(&doc.native_types, &options, &["Beam"])
            .unwrap();
    assert!(generated.source.starts_with(&ordinary.source));
    assert_eq!(generated.semantic_type_bytes, ordinary.semantic_type_bytes);
    let exercise = format!(
        "\nconst SOURCE:&str={SOURCE:?};\nmod allocation_probe {{ {} }}\n{}",
        include_str!("common/allocation_probe.rs"),
        EXERCISE
    );
    generated_rust::exercise(&generated.source, &exercise, 4, "prepared-native-decode");
}
#[test]
fn unsupported_sequence_and_missing_root_refuse_generation() {
    let doc = check_syntax_document(
        &parse_syntax_document("type Entries = sequence U64 <= 4\n"),
        &StartupCatalog::new(),
    )
    .unwrap();
    assert!(generate_rust_bindings_with_prepared_storage(
        &doc.native_types,
        &RustBindingOptions::default(),
        &["Entries"]
    )
    .is_err());
    assert!(generate_rust_bindings_with_prepared_storage(
        &document().native_types,
        &RustBindingOptions::default(),
        &["Absent"]
    )
    .is_err());
}
const EXERCISE: &str = r#"
#[global_allocator]
static ALLOCATOR:allocation_probe::Allocator=allocation_probe::Allocator;
#[cfg(test)] mod acceptance {
use super::*;
use conduit_plot::{*,rust_binding::*};
fn doc()->CheckedSyntaxDocument {check_syntax_document(&parse_syntax_document(SOURCE),&StartupCatalog::new()).unwrap()}
fn bound()->PreparedDecodeBounds {PreparedDecodeBounds{canonical_bytes:16384,leaf_steps:64,text_bytes:32}}
fn value()->Beam {Beam::new(Interval::new(7,2).unwrap(),Label::new("exact".into()).unwrap(),false,Relation::Left,-9,[1,2,3,4]).unwrap()}
fn admit(d:&CheckedSyntaxDocument)->PreparedNativeAdmission<'_>{
 let id=&d.native_types.iter().find(|n|n.name=="Beam").unwrap().identity;
 let mut a=PreparedNativeAdmission::new(d,id,PreparedNativeBounds{canonical_bytes:16384,scopes:64,invariants:64}).unwrap();
 a.begin(&value().encode().unwrap()).unwrap();
 while a.step().unwrap()!=PreparedNativeProgress::Admitted{} a
}
#[test]fn positive_all_fields_and_fixed_steps_no_growth(){
 let d=doc();let a=admit(&d);let proof=a.admitted().unwrap();
 let mut staging=Beam::prepare_storage(&d,bound()).unwrap();
 let capacity=staging.canonical_capacity();
 let ((actual,steps),growth)=allocation_probe::observe(||{
  let mut steps=0;loop{steps+=1;if staging.step(&proof).unwrap()==PreparedDecodeProgress::Complete{break;}}
  let actual=match staging.finish(&proof){Ok(v)=>v,Err(_)=>panic!("completion refused")}; (actual,steps)
 });
 assert_eq!(growth.allocations,0);assert_eq!(growth.reallocations,0);
 assert_eq!(actual,value());assert_eq!(steps,10);assert!(capacity>=16384);
}
#[test]fn foreign_witness_cancel_and_pressure_cannot_finish(){
 let d=doc();let other=doc();let a=admit(&d);let b=admit(&other);let proof=a.admitted().unwrap();let foreign=b.admitted().unwrap();
 let mut storage=Beam::prepare_storage(&d,bound()).unwrap();
 assert_eq!(storage.step(&foreign),Err(PreparedDecodeRefusal::ForeignWitness));
 assert!(storage.finish(&proof).is_err());
 let mut storage=Beam::prepare_storage(&d,bound()).unwrap();storage.step(&proof).unwrap();storage.cancel();
 assert_eq!(storage.step(&proof),Err(PreparedDecodeRefusal::Cancelled));assert!(storage.finish(&proof).is_err());
 let mut storage=Beam::prepare_storage(&d,PreparedDecodeBounds{canonical_bytes:1,leaf_steps:64,text_bytes:32}).unwrap();
 assert_eq!(storage.step(&proof),Err(PreparedDecodeRefusal::Pressure));assert!(storage.finish(&proof).is_err());
 assert!(Beam::prepare_storage(&d,PreparedDecodeBounds{canonical_bytes:16384,leaf_steps:1,text_bytes:32}).is_err());
}
#[test]fn changed_checked_law_cannot_prepare_and_incomplete_staging_cannot_finish(){
 let d=doc();let mut forged=doc();let i=forged.native_types.iter_mut().find(|n|n.name=="Interval").unwrap();i.invariants.clear();
 assert!(Beam::prepare_storage(&forged,bound()).is_err());
 let a=admit(&d);let proof=a.admitted().unwrap();let mut storage=Beam::prepare_storage(&d,bound()).unwrap();storage.step(&proof).unwrap();assert!(storage.finish(&proof).is_err());
}
#[test]fn actual_invalid_nested_law_never_supplies_decode_witness(){
 let d=doc();let root=value().into_structured().unwrap();
 let integer=|n:u64| primitive_into_structured(StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap(),&n).unwrap();
 let bad=StructuredInfoValue::record(Interval::semantic_type().unwrap(),vec![StructuredFieldValue::new("end",integer(2)).unwrap(),StructuredFieldValue::new("start",integer(7)).unwrap()]).unwrap();
 let StructuredInfoValueShape::Record(fields)=root.shape() else {panic!()};
 let fields=fields.iter().map(|f|StructuredFieldValue::new(f.name(),if f.name()=="interval"{bad.clone()}else{f.value().clone()}).unwrap()).collect();
 let bytes=StructuredInfoValue::record(Beam::semantic_type().unwrap(),fields).unwrap().canonical_bytes().unwrap();
 let id=&d.native_types.iter().find(|n|n.name=="Beam").unwrap().identity;
 let mut a=PreparedNativeAdmission::new(&d,id,PreparedNativeBounds{canonical_bytes:16384,scopes:64,invariants:64}).unwrap();a.begin(&bytes).unwrap();
 let failure=loop{match a.step(){Ok(PreparedNativeProgress::Pending)=>(),Ok(_)=>panic!("invalid nested law admitted"),Err(e)=>break e}};
 assert!(matches!(failure,PreparedNativeRefusal::Invariant{..}));assert!(a.admitted().is_err());
}

}
"#;
