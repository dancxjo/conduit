use conduit_creche_model::names::{NamingCatalog, NamingRefusal};
const UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

#[test]
fn native_suggestions_match_the_existing_browser_catalog_and_derivation() {
    let catalog = NamingCatalog::shared().unwrap();
    assert_eq!(catalog.version, 7);
    let expected = [
        "Lucius Livius Lentulus",
        "Feng Mingyu",
        "Ellis W. Henderson",
        "Manuel Enrique Gutiérrez Rojas",
        "Davíð Margrétardóttir",
        "Takahashi Mei",
        "Amin ibn Nadir Rahman",
        "Claude Barbier Perrin",
        "Theo John Owen",
        "Janet Rose Lewis",
        "Cho Seung-hyeon",
        "Đinh Gia Nga",
        "Fọláṣadé Ọní",
        "Roksolana Zozulia",
        "Avner ben Yehuda",
        "Nahom Alula Tsehay",
        "Rafael Luís Azevedo Machado",
        "Devan Murugan",
        "Nur Rahayu",
        "Elin Walters",
        "Zozan Silêmanî",
        "TARGUS TARGUS",
        "Arvar Brightheart",
    ];
    assert_eq!(catalog.systems.len(), expected.len());
    for (system, expected) in catalog.systems.iter().zip(expected) {
        assert_eq!(
            catalog.name_for(UUID, &system.id, 0).unwrap().name,
            expected
        );
    }
    for (variation, expected) in [
        "Gonçalo Pacheco Guerreiro",
        "Đoàn Hồng Dũng",
        "Caeluil Glenlight",
        "Matvii Pavlenko",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(
            catalog
                .name_for(UUID, "surprise", variation as u32)
                .unwrap()
                .name,
            *expected
        );
    }
    assert_eq!(
        catalog
            .name_for(UUID, "classic-anglophone", 62)
            .unwrap()
            .name,
        "Bob Joseph Young"
    );
    assert_eq!(
        catalog
            .name_for(UUID, "classic-anglophone", 105)
            .unwrap()
            .name,
        "Susan Irene Lewis"
    );
}

#[test]
fn invalid_identities_refuse_and_all_suggestions_fit_the_same_utf8_bound() {
    let catalog = NamingCatalog::shared().unwrap();
    assert_eq!(
        catalog.name_for("invalid", "surprise", 0),
        Err(NamingRefusal::Uuid)
    );
    assert_eq!(
        catalog.name_for(UUID, "unknown", 0),
        Err(NamingRefusal::UnknownSystem)
    );
    for system in &catalog.systems {
        for variation in 0..32 {
            let suggestion = catalog.name_for(UUID, &system.id, variation).unwrap();
            assert!(suggestion.name.len() <= 64);
            assert_eq!(
                suggestion,
                catalog
                    .name_for(&UUID.to_uppercase(), &system.id, variation)
                    .unwrap()
            );
        }
    }
}
