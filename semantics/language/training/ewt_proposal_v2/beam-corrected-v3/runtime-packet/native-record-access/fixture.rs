use conduit_core::{validate_canonical_structured_value,PreparedCanonicalRecordAccess};
use std::{hint::black_box,time::Instant};
fn main(){
 let bytes=std::fs::read("work/native-child-rank/epoch-0.bin").unwrap();
 let value=validate_canonical_structured_value(&bytes).unwrap();
 let bound=PreparedCanonicalRecordAccess::storage_bound(value.type_bytes()).unwrap();
 let access=PreparedCanonicalRecordAccess::prepare(value.type_bytes(),bound).unwrap();
 for name in ["candidate0","candidate1","candidate2","candidate3"]{assert_eq!(access.field(value,name).unwrap(),value.record_field(name).unwrap());}
 let mut times=[0;2];
 for mode in 0..2 {let started=Instant::now();for _ in 0..10000{for name in ["candidate0","candidate1","candidate2","candidate3"]{let child=if mode==0{value.record_field(black_box(name)).unwrap()}else{access.field(value,black_box(name)).unwrap()};black_box(child);}}times[mode]=started.elapsed().as_nanos();}
 println!("{{\"original_fixture_bytes\":{},\"complete_type_bytes\":{},\"recipe_requested_retained_bytes\":{},\"field_lookups_per_mode\":40000,\"reference_nanos\":{},\"prepared_nanos\":{},\"equal_full_views\":true}}",bytes.len(),value.type_bytes().len(),bound,times[0],times[1]);
}
