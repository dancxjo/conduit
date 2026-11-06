//! Actual optional/variant/sequence occurrences retain inactive typed storage.
use conduit_plot::{rust_binding::*, *};
#[path = "common/generated_rust.rs"]
mod generated_rust;
const SOURCE: &str = r#"
type Label = Text <= 16B not in [""]
type Interval = {
 start: U64
 end: U64
 where .start <= .end
}
type Row = {
 label: Label
 range: Interval?
}
type Records = {
 items: sequence Row in 1..=3
}
type Choice =
 quiet
 | records Records
type Batch = {
 rows: sequence Choice? <= 3
}
type Minimum = sequence U64 in 1..=3
"#;
#[test]
fn generated_nested_active_storage_and_no_std() {
    let d = check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let generated = generate_rust_bindings_with_profiled_storage(
        &d.native_types,
        &RustBindingOptions::default(),
        &["Batch", "Minimum"],
    )
    .unwrap();
    let exercise = format!(
        "\nconst SOURCE:&str={SOURCE:?};\nmod allocation_probe{{{}}}\n{}",
        include_str!("common/allocation_probe.rs"),
        EXERCISE
    );
    generated_rust::exercise(&generated.source, &exercise, 5, "profiled-native-nested");
}
const EXERCISE: &str = r#"
#[global_allocator]static ALLOCATOR:allocation_probe::Allocator=allocation_probe::Allocator;
#[cfg(test)]mod acceptance{
use super::*;use conduit_plot::{*,rust_binding::*};
fn doc()->CheckedSyntaxDocument{check_syntax_document(&parse_syntax_document(SOURCE),&StartupCatalog::new()).unwrap()}
fn bounds()->PreparedStorageBounds{PreparedStorageBounds{decode:PreparedDecodeBounds{canonical_bytes:32768,leaf_steps:256,text_bytes:65536},sequence_items:64,storage_bytes:262144}}
fn profile(d:&CheckedSyntaxDocument,name:&str,physical:usize,text:usize)->PreparedStorageProfile{
 let root=&d.native_types.iter().find(|n|n.name==name).unwrap().identity;let s=discover_prepared_storage_schema(&d.native_types,root).unwrap();
 PreparedStorageProfile{root:root.clone(),capacities:s.sites().iter().map(|c|PreparedStorageCapacity{path:c.path.clone(),limit:match c.limit{PreparedStorageLimit::SequenceItems(_)=>PreparedStorageLimit::SequenceItems(physical),PreparedStorageLimit::TextBytes(_)=>PreparedStorageLimit::TextBytes(text)}}).collect()}
}
fn value(outer:usize,inner:usize,label:&str,mode:usize)->Batch{
 let row=||Row::new(Label::new(label.into()).unwrap(),Some(Interval::new(7,2).unwrap())).unwrap();
 let rows=(0..outer).map(|_|match mode{0=>None,1=>Some(Choice::Quiet),_=>Some(Choice::Records(ChoiceRecords{items:BoundedSequence::try_from_iter((0..inner).map(|_|row())).unwrap()}))});
 Batch::new(BoundedSequence::try_from_iter(rows).unwrap()).unwrap()
}
fn admission<'d>(d:&'d CheckedSyntaxDocument,name:&str,bytes:&[u8])->PreparedNativeAdmission<'d>{let mut a=PreparedNativeAdmission::new(d,&profile(d,name,0,0).root,PreparedNativeBounds{canonical_bytes:32768,scopes:256,invariants:256,contracts:256}).unwrap();a.begin(bytes).unwrap();a}
fn admit<'d>(d:&'d CheckedSyntaxDocument,name:&str,bytes:&[u8])->PreparedNativeAdmission<'d>{let mut a=admission(d,name,bytes);while a.step().unwrap()!=PreparedNativeProgress::Admitted{}a}
fn quiet(c:allocation_probe::Observation){assert_eq!((c.allocations,c.reallocations,c.deallocations),(0,0,0));}
#[test]fn active_nested_elements_and_inactive_alternatives_keep_backing(){let d=doc();for mode in 0..3{
 let expected=value(2,2,"ββ",mode);let a=admit(&d,"Batch",&expected.clone().encode().unwrap());let w=a.admitted().unwrap();
 let mut s=Batch::prepare_profiled_storage(&d,&profile(&d,"Batch",2,4),bounds()).unwrap();assert_eq!(s.cost().text_bytes,16);assert_eq!(s.cost().sequence_items,6);
 let ((actual,retained),c)=allocation_probe::observe(||{while s.step(&w).unwrap()!=PreparedDecodeProgress::Complete{}s.finish_retained(&w).map_err(|e|e.0).unwrap()});quiet(c);assert_eq!(actual,expected);assert_eq!(retained.cost().text_bytes,16);
}}
#[test]fn nested_count_and_text_pressure_never_publish_or_release_staging(){let d=doc();for expected in [value(2,3,"ββ",2),value(2,2,"longer",2),Batch::new(BoundedSequence::try_from_iter([Some(Choice::Records(ChoiceRecords{items:BoundedSequence::try_from_iter([Row::new(Label::new("ok".into()).unwrap(),None).unwrap(),Row::new(Label::new("longer".into()).unwrap(),None).unwrap()]).unwrap()}))]).unwrap()).unwrap()]{
 let a=admit(&d,"Batch",&expected.encode().unwrap());let w=a.admitted().unwrap();let mut s=Batch::prepare_profiled_storage(&d,&profile(&d,"Batch",2,4),bounds()).unwrap();
 let ((reason,retained),c)=allocation_probe::observe(||{loop{match s.step(&w){Ok(PreparedDecodeProgress::Pending)=>(),Ok(_)=>panic!("excess capacity published"),Err(e)=>{assert_eq!(e,PreparedDecodeRefusal::Pressure);break}}}s.finish_retained(&w).err().unwrap()});quiet(c);assert_eq!(reason,PreparedDecodeRefusal::Pressure);assert_eq!(retained.cost().text_bytes,16);
}}
#[test]fn semantic_minimum_maximum_and_zero_physical_capacity_are_distinct(){let d=doc();
 for count in [1,3]{let expected=Minimum::new(BoundedSequence::try_from_iter((0..count).map(|i|i as u64)).unwrap()).unwrap();let a=admit(&d,"Minimum",&expected.clone().encode().unwrap());let w=a.admitted().unwrap();let mut s=Minimum::prepare_profiled_storage(&d,&profile(&d,"Minimum",3,0),bounds()).unwrap();let ((actual,_retained),c)=allocation_probe::observe(||{while s.step(&w).unwrap()!=PreparedDecodeProgress::Complete{}s.finish_retained(&w).map_err(|e|e.0).unwrap()});quiet(c);assert_eq!(actual,expected);}
 let expected=Minimum::new(BoundedSequence::try_from_iter([1]).unwrap()).unwrap();let a=admit(&d,"Minimum",&expected.encode().unwrap());let w=a.admitted().unwrap();let mut s=Minimum::prepare_profiled_storage(&d,&profile(&d,"Minimum",0,0),bounds()).unwrap();assert_eq!(s.step(&w),Err(PreparedDecodeRefusal::Pressure));assert!(s.finish_retained(&w).is_err());
 assert!(Minimum::new(BoundedSequence::new()).is_err());
 // Deliberately encode an empty collection under the actual nonempty semantic
 // Type; the existing law/shape admission must refuse, without a fake witness.
 let mut bytes=Minimum::semantic_type().unwrap().canonical_bytes().unwrap();bytes.push(1);bytes.extend_from_slice(&0u32.to_le_bytes());
 let mut a=PreparedNativeAdmission::new(&d,&profile(&d,"Minimum",0,0).root,PreparedNativeBounds{canonical_bytes:32768,scopes:256,invariants:256,contracts:256}).unwrap();
 let denied=match a.begin(&bytes){Err(_)=>true,Ok(())=>loop{match a.step(){Err(_)=>break true,Ok(PreparedNativeProgress::Pending)=>(),Ok(_)=>break false}}};assert!(denied);assert!(a.admitted().is_err());
}
#[test]fn changed_law_metadata_and_excessive_physical_templates_refuse_preparation(){let mut d=doc();let p=profile(&d,"Batch",2,4);d.native_types.iter_mut().find(|n|n.name=="Interval").unwrap().invariants.clear();assert!(Batch::prepare_profiled_storage(&d,&p,bounds()).is_err());
 let d=doc();let p=profile(&d,"Batch",3,16);let mut b=bounds();b.decode.text_bytes=143;assert!(Batch::prepare_profiled_storage(&d,&p,b).is_err()); // 3*3*16=144
 let mut wrong=p.clone();wrong.root=profile(&d,"Minimum",1,0).root;assert!(Batch::prepare_profiled_storage(&d,&wrong,bounds()).is_err());
}
#[test]fn invalid_late_nested_law_never_supplies_a_materialization_witness(){
 use conduit_core::{StructuredInfoType,StructuredInfoValue,StructuredInfoTypeShape as TS,StructuredFieldValue as F};
 let d=doc();let good=value(1,2,"ok",2).into_structured().unwrap();
 let field_type=|ty:StructuredInfoType,name:&str|{let TS::Record{fields,..}=ty.shape()else{panic!()};fields.iter().find(|f|f.name()==name).unwrap().value_type().clone()};
 let outer=field_type(Batch::semantic_type().unwrap(),"rows");let TS::Sequence{element:optional,..}=outer.shape()else{panic!()};
 let inner=field_type(Records::semantic_type().unwrap(),"items");let range=field_type(Row::semantic_type().unwrap(),"range");
 let number=|n:u64|primitive_into_structured(StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap(),&n).unwrap();
 let bad=StructuredInfoValue::record(Interval::semantic_type().unwrap(),vec![F::new("end",number(2)).unwrap(),F::new("start",number(7)).unwrap()]).unwrap();
 let bad_row=StructuredInfoValue::record(Row::semantic_type().unwrap(),vec![F::new("label",Label::new("ok".into()).unwrap().into_structured().unwrap()).unwrap(),F::new("range",StructuredInfoValue::variant(range,"some",bad).unwrap()).unwrap()]).unwrap();
 let first=Row::new(Label::new("ok".into()).unwrap(),Some(Interval::new(7,2).unwrap())).unwrap().into_structured().unwrap();
 let records=StructuredInfoValue::record(Records::semantic_type().unwrap(),vec![F::new("items",StructuredInfoValue::sequence(inner,vec![first,bad_row]).unwrap()).unwrap()]).unwrap();
 let choice=StructuredInfoValue::variant(Choice::semantic_type().unwrap(),"records",records).unwrap();let some=StructuredInfoValue::variant(optional.clone(),"some",choice).unwrap();
 let bytes=StructuredInfoValue::record(good.value_type().clone(),vec![F::new("rows",StructuredInfoValue::sequence(outer,vec![some]).unwrap()).unwrap()]).unwrap().canonical_bytes().unwrap();
 let mut a=admission(&d,"Batch",&bytes);let mut steps=0;let failure=loop{steps+=1;match a.step(){Ok(PreparedNativeProgress::Pending)=>(),Ok(_)=>panic!("bad late child admitted"),Err(e)=>break e}};
 assert!(steps>1);assert!(matches!(failure,PreparedNativeRefusal::Invariant{..}));assert!(a.admitted().is_err());
}
}
"#;
