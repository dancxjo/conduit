use super::*;

#[test]
fn unrelated_kind_edits_reuse_checked_and_expanded_owners() {
    let (startup, profile) = catalog("unit@1");
    let backs = Arc::new(CanonicalBackCatalog::new());
    let mut cache = cache();
    let original = cache.check(SOURCE, &startup).unwrap();
    let expanded = cache.expand(&original, "main", &profile, &backs).unwrap();
    let reads = original
        .checking_dependencies()
        .expect("bounded checking read set");
    assert!(reads
        .iter()
        .any(|read| read.table == CatalogueTable::Signature && read.key == "test/unit"));
    assert!(cache.expansion_dependencies(&expanded).is_some());
    let mut changed = (*profile).clone();
    let mut unrelated = kind("unrelated@1");
    unrelated.kind_id = kind_id("unrelated/new-kind");
    changed.insert_kind(unrelated).unwrap();
    let changed = Arc::new(changed);
    let changed_startup = Arc::new(changed.startup_catalog().unwrap());
    let current = cache.check(SOURCE, &changed_startup).unwrap();
    let current_expanded = cache.expand(&current, "main", &changed, &backs).unwrap();
    assert!(Arc::ptr_eq(original.checked(), current.checked()));
    assert!(Arc::ptr_eq(&expanded, &current_expanded));
    assert_eq!(
        **current.checked(),
        check_syntax_document(&parse_syntax_document(SOURCE), &changed_startup).unwrap()
    );
    assert_eq!(
        *current_expanded,
        cold(SOURCE, &changed_startup, "main", &changed, &backs)
    );
    assert_eq!(cache.stats().checks, 1);
    assert_eq!(cache.stats().expansions, 1);
}

#[test]
fn absent_import_resolution_is_a_dependency_even_when_local_plot_supplies_meaning() {
    let (startup, _) = catalog("unit@1");
    let source =
        "with helper as local\nplot helper {\n a: test/unit\n}\nplot main {\n h: local\n}\n";
    let mut cache = cache();
    let original = cache.check(source, &startup).unwrap();
    assert!(original
        .checking_dependencies()
        .unwrap()
        .iter()
        .any(|read| read.table == CatalogueTable::Signature && read.key == "helper"));
    let mut changed = (*startup).clone();
    changed
        .insert(KindSignature {
            kind: "helper".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let changed = Arc::new(changed);
    let expected = check_syntax_document(&parse_syntax_document(source), &changed);
    let result = cache.check(source, &changed);
    match (expected, result) {
        (Ok(expected), Ok(current)) => {
            assert_eq!(**current.checked(), expected);
            assert!(!Arc::ptr_eq(original.checked(), current.checked()));
        }
        (Err(expected), Err(actual)) => assert_eq!(actual, expected),
        _ => panic!("cache and cold import resolution disagreed"),
    }
    assert_eq!(cache.stats().checks, 2);
}

#[test]
fn fore_changes_invalidate_shorthand_binding_without_a_generation_key() {
    let make = |output_kind: &str| {
        let mut startup = StartupCatalog::new();
        startup
            .insert(KindSignature {
                kind: "test/unit".into(),
                startup_parameters: vec![],
            })
            .unwrap();
        let port = |name, kind, direction| PortDescriptor {
            port_id: port_id(name),
            value_kind: kind_id(kind),
            direction,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        };
        startup
            .insert_fore(
                "test/unit",
                CheckedFront::new(
                    vec![],
                    vec![port("input", "value/text", PortDirection::Input)],
                    vec![port("output", output_kind, PortDirection::Output)],
                    Some((port_id("input"), port_id("output"))),
                ),
            )
            .unwrap();
        Arc::new(startup)
    };
    let source = "sans glyphs\nwith test/unit as ^^\nplot main (\n >> input: Text\n output: Text >>\n) {\n input ^^ output\n}\n";
    let old = make("value/text");
    let mut cache = cache();
    let original = cache.check(source, &old).unwrap();
    assert!(original
        .checking_dependencies()
        .unwrap()
        .iter()
        .any(|read| matches!(
            read.table,
            CatalogueTable::Fore | CatalogueTable::VariadicFore
        ) && read.key == "test/unit"));
    let changed = make("value/u64");
    let expected = check_syntax_document(&parse_syntax_document(source), &changed);
    match (cache.check(source, &changed), expected) {
        (Ok(current), Ok(expected)) => {
            assert!(!Arc::ptr_eq(original.checked(), current.checked()));
            assert_eq!(**current.checked(), expected);
        }
        (Err(actual), Err(expected)) => assert_eq!(actual, expected),
        _ => panic!("incremental Fore binding disagreed with cold checking"),
    }
    assert_eq!(cache.stats().checks, 2);
}

#[test]
fn private_initial_bytes_and_type_context_are_not_hidden_by_kind_read_slicing() {
    let (startup, _) = catalog("unit@1");
    let mut cache = cache();
    let original = cache.check(SOURCE, &startup).unwrap();
    for byte in [1, 2] {
        let mut changed = (*startup).clone();
        changed
            .insert_exact_initial_info(
                kind_id("private/custom-info"),
                "private-token",
                vec![byte],
                |bytes| bytes.len() == 1,
            )
            .unwrap();
        changed
            .insert_structured_type(
                "PrivateAlias",
                StructuredInfoType::leaf(kind_id("value/text")).unwrap(),
            )
            .unwrap();
        let changed = Arc::new(changed);
        let current = cache.check(SOURCE, &changed).unwrap();
        assert!(!Arc::ptr_eq(original.checked(), current.checked()));
        assert_eq!(
            **current.checked(),
            check_syntax_document(&parse_syntax_document(SOURCE), &changed).unwrap()
        );
    }
    assert_eq!(cache.stats().checks, 3);
}
