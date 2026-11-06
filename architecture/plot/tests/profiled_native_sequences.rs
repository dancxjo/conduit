//! Current emitted Rust and no_std proof, not a hand-written decoder substitute.
use conduit_plot::{rust_binding::*, *};
#[path = "common/generated_rust.rs"]
mod generated_rust;
const SOURCE: &str = "type Batch = {\n rows: sequence U64 <= 4\n}\n";
#[test]
fn current_generated_sequence_storage_and_no_std() {
    let doc =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let options = RustBindingOptions::default();
    let plain = generate_rust_bindings(&doc.native_types, &options).unwrap();
    let generated =
        generate_rust_bindings_with_profiled_storage(&doc.native_types, &options, &["Batch"])
            .unwrap();
    assert!(generated.source.starts_with(&plain.source));
    assert_eq!(generated.semantic_type_bytes, plain.semantic_type_bytes);
    let exercise = format!(
        "\nconst SOURCE:&str={SOURCE:?};\nmod allocation_probe{{{}}}\n{}",
        include_str!("common/allocation_probe.rs"),
        EXERCISE
    );
    generated_rust::exercise(&generated.source, &exercise, 5, "profiled-native-sequences");
}
#[test]
fn prefixed_bindings_and_unsupported_options_have_explicit_boundaries() {
    let d = check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let options = RustBindingOptions {
        type_prefix: "checked".into(),
        ..RustBindingOptions::default()
    };
    let generated =
        generate_rust_bindings_with_profiled_storage(&d.native_types, &options, &["Batch"])
            .unwrap();
    let exercise=format!("\nconst SOURCE:&str={SOURCE:?};\n#[test]fn prepare_prefixed(){{use conduit_plot::{{*,rust_binding::*}};let d=check_syntax_document(&parse_syntax_document(SOURCE),&StartupCatalog::new()).unwrap();let root=d.native_types.iter().find(|n|n.name==\"Batch\").unwrap();let schema=discover_prepared_storage_schema(&d.native_types,&root.identity).unwrap();let p=PreparedStorageProfile{{root:root.identity.clone(),capacities:schema.sites().to_vec()}};assert!(CheckedBatch::prepare_profiled_storage(&d,&p,PreparedStorageBounds{{decode:PreparedDecodeBounds{{canonical_bytes:1024,leaf_steps:64,text_bytes:0}},sequence_items:4,storage_bytes:65536}}).is_ok());}}");
    generated_rust::exercise(&generated.source, &exercise, 1, "profiled-native-prefix");
    assert!(generate_rust_bindings_with_profiled_storage(
        &d.native_types,
        &options,
        &["Batch", "Batch"]
    )
    .is_err());
    assert!(
        generate_rust_bindings_with_profiled_storage(&d.native_types, &options, &["Absent"])
            .is_err()
    );
}
const EXERCISE: &str = r#"
#[global_allocator]static ALLOCATOR:allocation_probe::Allocator=allocation_probe::Allocator;
#[cfg(test)]mod acceptance{
use super::*;use conduit_plot::{*,rust_binding::*};
fn doc()->CheckedSyntaxDocument{check_syntax_document(&parse_syntax_document(SOURCE),&StartupCatalog::new()).unwrap()}
fn bounds()->PreparedStorageBounds{PreparedStorageBounds{decode:PreparedDecodeBounds{canonical_bytes:16384,leaf_steps:64,text_bytes:0},sequence_items:4,storage_bytes:65536}}
fn profile(d:&CheckedSyntaxDocument,count:usize)->PreparedStorageProfile{
 let root=&d.native_types.iter().find(|n|n.name=="Batch").unwrap().identity;
 let schema=discover_prepared_storage_schema(&d.native_types,root).unwrap();
 PreparedStorageProfile{root:root.clone(),capacities:schema.sites().iter().map(|site|PreparedStorageCapacity{path:site.path.clone(),limit:PreparedStorageLimit::SequenceItems(count)}).collect()}
}
fn value(count:usize)->Batch{Batch::new(BoundedSequence::try_from_iter((0..count).map(|v|v as u64)).unwrap()).unwrap()}
fn admit(d:&CheckedSyntaxDocument,count:usize)->PreparedNativeAdmission<'_>{
 let mut a=PreparedNativeAdmission::new(d,&profile(d,0).root,PreparedNativeBounds{canonical_bytes:16384,scopes:64,invariants:64,contracts:64}).unwrap();a.begin(&value(count).encode().unwrap()).unwrap();while a.step().unwrap()!=PreparedNativeProgress::Admitted{}a
}
fn quiet(c:allocation_probe::Observation){assert_eq!((c.allocations,c.reallocations,c.deallocations),(0,0,0));}
#[test]fn empty_actual_and_full_physical_sequences_keep_all_backing(){
 let d=doc();for (physical,actual)in [(0,0),(2,0),(2,1),(2,2),(4,4)]{
 let requested=profile(&d,physical);let mut s=Batch::prepare_profiled_storage(&d,&requested,bounds()).unwrap();assert_eq!(s.profile(),&requested);
 let expected=value(actual);let a=admit(&d,actual);let w=a.admitted().unwrap();
 let ((v,retained,steps),c)=allocation_probe::observe(||{let mut steps=0;loop{steps+=1;if s.step(&w).unwrap()==PreparedDecodeProgress::Complete{break}}let(v,r)=s.finish_retained(&w).map_err(|e|e.0).unwrap();(v,r,steps)});
 quiet(c);assert_eq!(v,expected);assert_eq!(v.rows.physical_capacity(),physical);assert!(steps<=retained.cost().maximum_steps);assert_eq!(retained.profile(),&requested);
 }
}
#[test]fn excess_actual_length_refuses_before_any_element_and_preserves_pending(){
 let d=doc();let mut s=Batch::prepare_profiled_storage(&d,&profile(&d,2),bounds()).unwrap();let a=admit(&d,3);let w=a.admitted().unwrap();
 let ((reason,mut s),c)=allocation_probe::observe(||{assert_eq!(s.step(&w),Err(PreparedDecodeRefusal::Pressure));s.finish_retained(&w).err().unwrap()});quiet(c);assert_eq!(reason,PreparedDecodeRefusal::Pressure);assert_eq!(s.profile().capacities[0].limit,PreparedStorageLimit::SequenceItems(2));assert_eq!(s.step(&w),Err(PreparedDecodeRefusal::Pressure));
}
#[test]fn lower_actual_work_budget_and_canonical_pressure_preserve_staging(){
 let d=doc();for canonical in [1,16384]{let mut b=bounds();b.decode.canonical_bytes=canonical;b.decode.leaf_steps=1;
 let mut s=Batch::prepare_profiled_storage(&d,&profile(&d,4),b).unwrap();let a=admit(&d,2);let w=a.admitted().unwrap();let ((reason,_s),c)=allocation_probe::observe(||{while s.step(&w)==Ok(PreparedDecodeProgress::Pending){}s.finish_retained(&w).err().unwrap()});quiet(c);assert_eq!(reason,PreparedDecodeRefusal::Pressure);}
 // A one-step empty sequence can finish despite a larger physical template.
 let mut b=bounds();b.decode.leaf_steps=1;let mut s=Batch::prepare_profiled_storage(&d,&profile(&d,4),b).unwrap();let a=admit(&d,0);let w=a.admitted().unwrap();assert_eq!(s.step(&w),Ok(PreparedDecodeProgress::Complete));assert!(s.finish_retained(&w).is_ok());
}
#[test]fn changed_foreign_cancelled_and_reused_retained_storage_cannot_publish(){
 let d=doc();let other=doc();let a=admit(&d,1);let changed=admit(&d,2);let foreign=admit(&other,1);let w=a.admitted().unwrap();
 for wrong in [changed.admitted().unwrap(),foreign.admitted().unwrap()]{let mut s=Batch::prepare_profiled_storage(&d,&profile(&d,4),bounds()).unwrap();let ((reason,_s),c)=allocation_probe::observe(||{s.step(&w).unwrap();assert_eq!(s.step(&wrong),Err(PreparedDecodeRefusal::ForeignWitness));s.finish_retained(&w).err().unwrap()});quiet(c);assert_eq!(reason,PreparedDecodeRefusal::ForeignWitness);}
 let mut s=Batch::prepare_profiled_storage(&d,&profile(&d,4),bounds()).unwrap();s.step(&w).unwrap();s.cancel();assert_eq!(s.step(&w),Err(PreparedDecodeRefusal::Cancelled));assert!(s.finish_retained(&w).is_err());
 let mut s=Batch::prepare_profiled_storage(&d,&profile(&d,4),bounds()).unwrap();while s.step(&w).unwrap()!=PreparedDecodeProgress::Complete{}let(_v,mut r)=s.finish_retained(&w).map_err(|e|e.0).unwrap();assert_eq!(r.step(&w),Err(PreparedDecodeRefusal::State));assert!(r.finish_retained(&w).is_err());
}
#[test]fn invalid_profiles_and_finite_storage_budgets_refuse_preparation(){
 let d=doc();let p=profile(&d,2);let mut invalid=p.clone();invalid.capacities[0].path.push(NativePathStep::Each);assert!(Batch::prepare_profiled_storage(&d,&invalid,bounds()).is_err());
 let mut duplicate=p.clone();duplicate.capacities.push(duplicate.capacities[0].clone());assert!(Batch::prepare_profiled_storage(&d,&duplicate,bounds()).is_err());
 assert!(Batch::prepare_profiled_storage(&d,&profile(&d,5),bounds()).is_err());
 for storage in [0,1,64]{let mut b=bounds();b.storage_bytes=storage;assert!(Batch::prepare_profiled_storage(&d,&p,b).is_err());}
 let mut b=bounds();b.sequence_items=1;assert!(Batch::prepare_profiled_storage(&d,&p,b).is_err());
}
}
"#;
