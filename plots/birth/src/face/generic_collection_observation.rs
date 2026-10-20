//! Retained actual generic views/proposals, independent of the linear reader.
use super::*;
use serde_json::{Value, json};

pub(super) fn observe(face: &Face) -> Value {
    let show = fixtures::acknowledged_fixture_show(face);
    let collections: Vec<_> = [
        "/reviewed-plots",
        "/name-system/input-details",
        "/initial-plots",
    ]
    .into_iter()
    .map(|suffix| {
        let id = &collection(face, suffix).identity;
        let view = prepare_face_collection(face, id, 64).unwrap();
        let source = view.source();
        let mut subjects: Vec<_> = view.items().iter().map(|s| s.identity.as_str()).collect();
        subjects.push(view.subject().identity.as_str());
        // The exclusive action's authoritative current value is on its
        // explicitly associated value-source, not on the collection itself.
        for p in source.properties.iter().filter(|p| p.subject == *id) {
            if p.name == "value-source" {
                let FacePropertyValue::Identity(value_source) = &p.value else {
                    panic!()
                };
                subjects.push(value_source);
            }
        }
        let properties: Vec<_> = source
            .properties
            .iter()
            .filter(|p| subjects.contains(&p.subject.as_str()))
            .collect();
        let actions: Vec<_> = source
            .actions
            .iter()
            .filter(|a| subjects.contains(&a.target.as_str()))
            .collect();
        let pages: Vec<_> = (0..=view.items().len())
            .map(|first| {
                let page = view.page(first, 3).unwrap();
                json!({"first":first, "maximum":3, "source_face":page.face_identity,
                "revision":page.revision, "collection":page.collection_identity,
                "items":page.items, "total":page.total, "next":page.next})
            })
            .collect();
        let proposals: Vec<_> = view
            .items()
            .iter()
            .flat_map(|item| {
                [false, true].map(|choose| {
                    let outcome = match view.interaction(source, &show, &item.identity, choose, 1) {
                        Ok(interaction) => json!({"checked_proposal":interaction}),
                        Err(reason) => json!({"refused":format!("{reason:?}")}),
                    };
                    json!({"item":item.identity,"choose":choose,"outcome":outcome})
                })
            })
            .collect();
        let multiplicity = view.multiplicity().map(|mode| match mode {
            conduit_face::ChoiceMultiplicity::Exclusive => "exclusive",
            conduit_face::ChoiceMultiplicity::Independent => "independent",
        });
        json!({"source_face":source.identity,"revision":source.revision,
            "subject":view.subject(),"multiplicity":multiplicity,
            "ordered_items":view.items(),"pages":pages,"proposals":proposals,
            "source_properties":properties,"source_actions":actions})
    })
    .collect();
    json!({"proof_class":"source-derived deterministic generic observation; proposals only, no execution",
        "source_face":face.identity,"revision":face.revision,"show":show,"collections":collections})
}
