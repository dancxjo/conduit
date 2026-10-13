use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);
static REQUESTED: AtomicUsize = AtomicUsize::new(0);
struct Probe;
unsafe impl GlobalAlloc for Probe {
 unsafe fn alloc(&self,l:Layout)->*mut u8 { let p=System.alloc(l);if !p.is_null(){REQUESTED.fetch_add(l.size(),Ordering::SeqCst);let now=LIVE.fetch_add(l.size() as isize,Ordering::SeqCst)+l.size() as isize;PEAK.fetch_max(now,Ordering::SeqCst);}p }
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){LIVE.fetch_sub(l.size() as isize,Ordering::SeqCst);System.dealloc(p,l)}
 unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8 {let q=System.realloc(p,l,n);if !q.is_null(){REQUESTED.fetch_add(n,Ordering::SeqCst);let now=LIVE.fetch_add(n as isize-l.size() as isize,Ordering::SeqCst)+n as isize-l.size() as isize;PEAK.fetch_max(now,Ordering::SeqCst);}q}
}
#[global_allocator] static ALLOCATOR:Probe=Probe;
fn main(){
 use conduit_plot::rust_binding::{PreparedNativeFamily,PreparedNativeFamilyLimits};
 let original=std::fs::read("/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907/outputs/early-verb-public-pronunciation/generated-lexical-fact.bin").unwrap();
 let limits=PreparedNativeFamilyLimits {maximum_types:64,maximum_laws_per_type:64,maximum_input_bytes:conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES,maximum_retained_bytes:256*1024*1024,maximum_preparation_peak_bytes:512*1024*1024,maximum_conversion_requested_bytes:1024*1024*1024};
 let before=LIVE.load(Ordering::SeqCst);PEAK.store(before,Ordering::SeqCst);
 let start=std::time::Instant::now();
 let mut family=PreparedNativeFamily::prepare(PREPARED_NATIVE_FAMILY_ROOTS,limits).unwrap();
 let preparation_us=start.elapsed().as_micros();
 let retained=LIVE.load(Ordering::SeqCst)-before;let peak=PEAK.load(Ordering::SeqCst)-before;
 let receipt=family.storage_receipt();assert_eq!(receipt.types,35);
 assert!(receipt.retained_heap_bytes_bound>=retained as usize);assert!(receipt.preparation_peak_heap_bytes_bound>=peak as usize);
 eprintln!("actual_family_types={} retained={} retained_bound={} preparation_peak={} preparation_peak_bound={} conversion_requested_bound={} preparation_us={}",receipt.types,retained,receipt.retained_heap_bytes_bound,peak,receipt.preparation_peak_heap_bytes_bound,receipt.conversion_requested_bytes_bound,preparation_us);
 REQUESTED.store(0,Ordering::SeqCst);let start=std::time::Instant::now();
 let prepared=family.decode::<LanguageParserWindow8StableLexicalFact>(&original).unwrap();
 let prepared_us=start.elapsed().as_micros();let conversion_requested=REQUESTED.load(Ordering::SeqCst);
 assert!(conversion_requested<=receipt.conversion_requested_bytes_bound);
 eprintln!("actual_complete_recursive_prepared_us={} conversion_requested={}",prepared_us,conversion_requested);
 let start=std::time::Instant::now();let reference=LanguageParserWindow8StableLexicalFact::decode(&original).unwrap();
 let reference_us=start.elapsed().as_micros();assert_eq!(prepared,reference);drop(prepared);drop(reference);
 eprintln!("actual_complete_recursive_reference_us={}",reference_us);
 let value=StructuredInfoValue::from_canonical_bytes(&original).unwrap();
 let negative=replace_zero(&value,&["query","snapshot","candidate3","hypothesis","choices","1"]).canonical_bytes().unwrap();
 let expected=LanguageParserWindow8StableLexicalFact::decode(&negative);assert!(expected.is_err());
 assert_eq!(family.decode::<LanguageParserWindow8StableLexicalFact>(&negative),expected);
 eprintln!("actual_derived_negative_refusal={:?}",expected.err());
 assert!(family.decode::<LanguageParserWindow8FactQuery>(&original).is_err());
 let mut malformed=original.clone();malformed.pop();assert!(family.decode::<LanguageParserWindow8StableLexicalFact>(&malformed).is_err());
 assert!(PreparedNativeFamily::prepare(PREPARED_NATIVE_FAMILY_ROOTS,PreparedNativeFamilyLimits{maximum_retained_bytes:receipt.retained_heap_bytes_bound-1,..limits}).is_err());
 assert!(PreparedNativeFamily::prepare(PREPARED_NATIVE_FAMILY_ROOTS,PreparedNativeFamilyLimits{maximum_preparation_peak_bytes:receipt.preparation_peak_heap_bytes_bound-1,..limits}).is_err());
 assert!(PreparedNativeFamily::prepare(PREPARED_NATIVE_FAMILY_ROOTS,PreparedNativeFamilyLimits{maximum_conversion_requested_bytes:receipt.conversion_requested_bytes_bound-1,..limits}).is_err());
 assert_eq!(std::fs::read("/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907/outputs/early-verb-public-pronunciation/generated-lexical-fact.bin").unwrap(),original);
 eprintln!("actual complete Source family/reference/quota/foreign/malformed parity PASS");
}

fn replace_zero(
    value: &conduit_core::StructuredInfoValue,
    path: &[&str],
) -> conduit_core::StructuredInfoValue {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoValue as V, StructuredInfoValueShape as S,
    };
    if path.is_empty() {
        let S::Leaf(bytes) = value.shape() else {
            panic!("choice is primitive")
        };
        assert_ne!(bytes, &vec![0; bytes.len()]);
        return V::leaf(value.value_type().clone(), vec![0; bytes.len()]).unwrap();
    }
    match value.shape() {
        S::Record(fields) => V::record(
            value.value_type().clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        if field.name() == path[0] {
                            replace_zero(field.value(), &path[1..])
                        } else {
                            field.value().clone()
                        },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        S::Collection(values) => {
            let index: usize = path[0].parse().unwrap();
            V::collection(
                value.value_type().clone(),
                values
                    .iter()
                    .enumerate()
                    .map(|(i, value)| {
                        if i == index {
                            replace_zero(value, &path[1..])
                        } else {
                            value.clone()
                        }
                    })
                    .collect(),
            )
            .unwrap()
        }
        _ => panic!("fixture path has no selected field"),
    }
}
