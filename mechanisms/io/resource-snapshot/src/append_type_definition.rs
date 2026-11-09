//! Build-host definition of the one canonical generic try-append outcome.
use alloc::vec;
use conduit_core::{kind_id, StructuredInfoType, StructuredVariantCase};
pub fn definition() -> StructuredInfoType {
    let index = StructuredInfoType::leaf(kind_id("value/u64")).expect("U64");
    let unit = StructuredInfoType::leaf(kind_id("value/empty")).expect("Empty");
    StructuredInfoType::variant(
        kind_id("resource/try-append-record/result@1"),
        vec![
            StructuredVariantCase::new("recorded", index).expect("recorded"),
            StructuredVariantCase::new("capacity", unit.clone()).expect("capacity"),
            StructuredVariantCase::new("uncertain", unit.clone()).expect("uncertain"),
            StructuredVariantCase::new("recovery-required", unit).expect("recovery"),
        ],
    )
    .expect("finite try-append result")
}
