use conduit_core::StructuredInfoTypeShape;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;

#[test]
fn universal_bases_have_exact_native_vocabulary_and_round_trip() {
    use LanguageUniversalDependencyRelation as R;
    let values = [
        ("acl", R::acl()),
        ("advcl", R::advcl()),
        ("advmod", R::advmod()),
        ("amod", R::amod()),
        ("appos", R::appos()),
        ("aux", R::aux()),
        ("case", R::case()),
        ("cc", R::cc()),
        ("ccomp", R::ccomp()),
        ("clf", R::clf()),
        ("compound", R::compound()),
        ("conj", R::conj()),
        ("cop", R::cop()),
        ("csubj", R::csubj()),
        ("dep", R::dep()),
        ("det", R::det()),
        ("discourse", R::discourse()),
        ("dislocated", R::dislocated()),
        ("expl", R::expl()),
        ("fixed", R::fixed()),
        ("flat", R::flat()),
        ("goeswith", R::goeswith()),
        ("iobj", R::iobj()),
        ("list", R::list()),
        ("mark", R::mark()),
        ("nmod", R::nmod()),
        ("nsubj", R::nsubj()),
        ("nummod", R::nummod()),
        ("obj", R::obj()),
        ("obl", R::obl()),
        ("orphan", R::orphan()),
        ("parataxis", R::parataxis()),
        ("punct", R::punct()),
        ("reparandum", R::reparandum()),
        ("root", R::root()),
        ("vocative", R::vocative()),
        ("xcomp", R::xcomp()),
    ];
    let native = universal_dependency_relation_type();
    let StructuredInfoTypeShape::Variant { cases, .. } = native.shape() else {
        panic!()
    };
    assert_eq!(cases.len(), values.len());
    for (name, value) in values {
        assert!(cases.iter().any(|case| case.tag() == name));
        assert_eq!(
            R::from_structured(value.into_structured().unwrap()).unwrap(),
            value
        );
    }
}
#[test]
fn subtypes_are_explicit_bounded_native_suffixes_and_catalog_types_check() {
    let subtype = LanguageDependencySubtype::new("pass".into()).unwrap();
    let relation = LanguageDependencyRelation::new(
        LanguageUniversalDependencyRelation::nsubj(),
        Some(subtype),
    )
    .unwrap();
    assert_eq!(
        LanguageDependencyRelation::from_structured(relation.clone().into_structured().unwrap())
            .unwrap(),
        relation
    );
    assert!(LanguageDependencySubtype::new("".into()).is_err());
    assert!(LanguageDependencySubtype::new("x".repeat(33)).is_err());
    let plain =
        LanguageDependencyRelation::new(LanguageUniversalDependencyRelation::vocative(), None)
            .unwrap();
    assert!(plain.subtype().is_none());
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(
            "type UdEncounter = {\n    relation: LanguageDependencyRelation\n}\n",
        ),
        &startup,
    );
    assert!(checked.is_ok(), "{checked:?}");
}
