use conduit_form::rust_binding::NativeRustBinding;
use conduit_time::TemplateCollectionRefusal;

#[test]
fn template_collection_refusals_round_trip_through_their_exact_native_type() {
    for refusal in [
        TemplateCollectionRefusal::Malformed,
        TemplateCollectionRefusal::NameEmpty,
        TemplateCollectionRefusal::NameTooLong,
        TemplateCollectionRefusal::DuplicateName,
        TemplateCollectionRefusal::CollectionFull,
        TemplateCollectionRefusal::NotFound,
        TemplateCollectionRefusal::CorruptTemplate,
    ] {
        let structured = refusal.into_structured().unwrap();
        assert_eq!(
            TemplateCollectionRefusal::from_structured(structured).unwrap(),
            refusal
        );
    }
}
