#![cfg(target_has_atomic = "ptr")]
use conduit_core::*;
use conduit_plot::*;
const VALID: &str = "# α 🦀\nplot main {\n a: test/unit\n}\n";
fn revision() -> CheckContextRevision {
    CheckContextRevision {
        grammar_edition: "conduitese-v1".into(),
        catalog: "fixture/catalog@1".into(),
        kinds_fores: "fixture/unit@1".into(),
        types_laws: "empty@1".into(),
        backs: "empty@1".into(),
    }
}
fn profile() -> ProfileCatalog {
    let mut p = ProfileCatalog::new();
    p.insert_kind(Kind {
        kind_id: kind_id("test/unit"),
        kind_contract_revision: "unit@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: vec![],
        outputs: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 64,
        },
    })
    .unwrap();
    p
}
fn projection(source: &str) -> SourceCheckProjection {
    let p = profile();
    let context = SourceCheckContext::new(
        revision(),
        p.startup_catalog().unwrap(),
        p,
        CanonicalBackCatalog::new(),
    )
    .unwrap();
    SourceCheckProjection::new("fixture/source-owner", 7, source, context).unwrap()
}
fn request(p: &SourceCheckProjection) -> SourceCheckRequest {
    SourceCheckRequest {
        snapshot: p.snapshot().clone(),
        context: p.context().clone(),
        entry: "main".into(),
        limits: SourceCheckLimits::default(),
    }
}
#[test]
fn real_checker_and_expander_produce_stable_descriptive_identities() {
    let mut p = projection(VALID);
    let r = request(&p);
    let a = p.project(&r).unwrap();
    assert_eq!(a, p.project(&r).unwrap());
    assert_eq!(a.schema_version, 1);
    let wire = serde_json::to_value(&a).unwrap();
    assert_eq!(wire["schema_version"], 1);
    assert_eq!(wire["plan"], "absent");
    assert_eq!(wire["play"], "absent");
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&p.project(&r).unwrap()).unwrap()
    );
    let catalog = profile();
    let checked = check_syntax_document(
        &parse_syntax_document(VALID),
        &catalog.startup_catalog().unwrap(),
    )
    .unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "main", &catalog).unwrap();
    assert_eq!(
        a.snapshot.source_document_id,
        checked.source_document_id.as_str()
    );
    assert_eq!(
        a.candidate,
        CandidateIdentity {
            checked_plot_id: Some(expanded.expanded.checked_plot_id.as_str().into()),
            expanded_plot_id: Some(expanded.expanded.expanded_plot_id.as_str().into())
        }
    );
    assert_eq!(a.check, CheckOutcome::Accepted);
    assert_eq!(a.plan, RealizationPresence::Absent);
    assert_eq!(a.play, RealizationPresence::Absent);
    let DependencyProvenance::Observed {
        checking,
        expansion,
    } = a.dependencies
    else {
        panic!("expected actual reference trace")
    };
    assert!(checking.iter().any(|r| r.key == "test/unit"));
    assert!(expansion.iter().any(|r| r.key == "test/unit"));
}
#[test]
fn real_authored_error_has_normalized_unicode_safe_source_span() {
    let source = "# α 🦀\nplot main {\n α: unknown/kind\n}\n";
    let mut p = projection(source);
    let info = p.project(&request(&p)).unwrap();
    assert_eq!(info.check, CheckOutcome::Rejected);
    assert_eq!(info.candidate, CandidateIdentity::default());
    assert_eq!(info.diagnostics.len(), 1);
    let d = &info.diagnostics[0];
    let s = d.primary_span.unwrap();
    assert!(source.get(s.start..s.end).is_some());
    assert_eq!(s, source_span(source, s.start..s.end).unwrap().into());
    assert_eq!(d.source_document_id, p.snapshot().source_document_id);
    assert_eq!(d.content_hash, p.snapshot().sha256);
    assert_eq!(s.line, 3);
}
#[test]
fn wrong_source_and_unknown_context_refuse_before_checking() {
    let mut p = projection(VALID);
    let good = request(&p);
    for field in 0..4 {
        let mut r = good.clone();
        match field {
            0 => r.snapshot.generation += 1,
            1 => r.snapshot.owner.push('x'),
            2 => r.snapshot.sha256.push('x'),
            _ => r.snapshot.source_document_id.push('x'),
        }
        assert_eq!(p.project(&r), Err(SourceCheckRefusal::StaleSource));
    }
    let mut r = good.clone();
    r.context.grammar_edition = "unknown".into();
    assert_eq!(p.project(&r), Err(SourceCheckRefusal::UnknownGrammar));
    for field in 0..4 {
        let mut r = good.clone();
        match field {
            0 => r.context.catalog.push('x'),
            1 => r.context.kinds_fores.push('x'),
            2 => r.context.types_laws.push('x'),
            _ => r.context.backs.push('x'),
        }
        assert_eq!(p.project(&r), Err(SourceCheckRefusal::UnknownContext));
    }
    assert_eq!(p.project(&good).unwrap().check, CheckOutcome::Accepted);
}
#[test]
fn missing_entry_is_not_a_partial_success_or_invented_candidate() {
    let mut p = projection(VALID);
    let mut r = request(&p);
    r.entry = "absent".into();
    let info = p.project(&r).unwrap();
    assert_eq!(info.check, CheckOutcome::Accepted);
    assert_eq!(info.candidate, CandidateIdentity::default());
    assert!(!info.diagnostics.is_empty());
    assert_eq!(info.diagnostics[0].primary_span, None);
    assert_eq!(info.dependencies, DependencyProvenance::WholeContext);
    r.entry = "main".into();
    assert!(p.project(&r).unwrap().candidate.expanded_plot_id.is_some());
}
#[test]
fn diagnostic_and_dependency_pressure_refuse_without_a_truncated_result() {
    let mut p = projection("plot main { a: unknown/kind }");
    let mut r = request(&p);
    r.limits.diagnostics = 0;
    assert_eq!(p.project(&r), Err(SourceCheckRefusal::DiagnosticOverflow));
    r.limits = SourceCheckLimits::default();
    r.limits.diagnostic_bytes = 1;
    assert_eq!(p.project(&r), Err(SourceCheckRefusal::DiagnosticOverflow));
    let mut p = projection(VALID);
    let mut r = request(&p);
    r.limits.dependencies = 0;
    assert_eq!(p.project(&r), Err(SourceCheckRefusal::DependencyOverflow));
    r.limits = SourceCheckLimits::default();
    r.limits.dependency_bytes = 1;
    assert_eq!(p.project(&r), Err(SourceCheckRefusal::DependencyOverflow));
    r.limits = SourceCheckLimits::default();
    assert_eq!(p.project(&r).unwrap().check, CheckOutcome::Accepted);
}
#[test]
fn malformed_request_and_snapshot_limits_are_explicit() {
    let mut p = projection(VALID);
    let mut r = request(&p);
    r.entry.clear();
    assert_eq!(p.project(&r), Err(SourceCheckRefusal::EntryBytes));
    r.entry = "main".into();
    r.limits.diagnostics = MAX_CHECK_DIAGNOSTICS + 1;
    assert_eq!(p.project(&r), Err(SourceCheckRefusal::Limits));
    let catalog = profile();
    let mut rev = revision();
    rev.grammar_edition = "future".into();
    assert!(matches!(
        SourceCheckContext::new(
            rev,
            catalog.startup_catalog().unwrap(),
            catalog,
            CanonicalBackCatalog::new()
        ),
        Err(SourceCheckRefusal::UnknownGrammar)
    ));
}

#[test]
fn portable_types_are_real_checked_native_declarations() {
    let source = include_str!("../../../semantics/language/source_check_info.conduit");
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    assert!(checked
        .native_types
        .iter()
        .any(|t| t.name == "SourceCheckInfoV1"));
    assert!(checked
        .native_types
        .iter()
        .any(|t| t.name == "SourceCheckRefusal"));
    assert!(
        checked.plots.is_empty(),
        "contract Types do not install a compiler implementation"
    );
}

#[test]
fn imported_native_law_context_is_retained_and_missing_import_cannot_fake_success() {
    let type_source = "type Timing = {\n denominator: U64\n where .denominator > 0\n}\n";
    let owner =
        check_syntax_document(&parse_syntax_document(type_source), &StartupCatalog::new()).unwrap();
    let profile = profile();
    let mut startup = profile.startup_catalog().unwrap();
    startup
        .insert_native_law_witness("owner/Timing", owner.native_law_witness("Timing").unwrap())
        .unwrap();
    let before = startup.clone();
    let mut rev = revision();
    rev.types_laws = owner.source_document_id.as_str().into();
    let source = "with owner/Timing as Imported\ntype Holder = {\n value: Imported\n}\nplot main {\n a: test/unit\n}\n";
    let ctx = SourceCheckContext::new(
        rev.clone(),
        startup.clone(),
        profile.clone(),
        CanonicalBackCatalog::new(),
    )
    .unwrap();
    let mut p = SourceCheckProjection::new("consumer", 1, source, ctx).unwrap();
    let r = request(&p);
    let projected = p.project(&r).unwrap();
    assert_eq!(projected.check, CheckOutcome::Accepted);
    assert!(projected.candidate.expanded_plot_id.is_some());
    assert_eq!(
        projected.context.types_laws,
        owner.source_document_id.as_str()
    );
    assert_eq!(
        startup, before,
        "extraction cannot mutate caller catalog or law custody"
    );
    assert_eq!(profile, self::profile());
    let mut missing_rev = rev;
    missing_rev.types_laws = "missing@2".into();
    let ctx = SourceCheckContext::new(
        missing_rev,
        profile.startup_catalog().unwrap(),
        profile,
        CanonicalBackCatalog::new(),
    )
    .unwrap();
    let mut missing = SourceCheckProjection::new("consumer", 1, source, ctx).unwrap();
    let no_import = missing.project(&request(&missing)).unwrap();
    assert_eq!(no_import.check, CheckOutcome::Rejected);
    assert_eq!(no_import.candidate, CandidateIdentity::default());
    assert!(!no_import.diagnostics.is_empty());
    assert_eq!(
        p.project(&r).unwrap(),
        projected,
        "other context cannot replace retained context"
    );
}

#[test]
fn changed_actual_catalog_revision_changes_candidate_without_changing_source() {
    let mut first = projection(VALID);
    let a = first.project(&request(&first)).unwrap();
    let mut p = ProfileCatalog::new();
    p.insert_kind(Kind {
        kind_id: kind_id("test/unit"),
        kind_contract_revision: "unit@2".into(),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: vec![],
        outputs: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 64,
        },
    })
    .unwrap();
    let mut rev = revision();
    rev.kinds_fores = "fixture/unit@2".into();
    rev.catalog = "fixture/catalog@2".into();
    let context = SourceCheckContext::new(
        rev,
        p.startup_catalog().unwrap(),
        p,
        CanonicalBackCatalog::new(),
    )
    .unwrap();
    let mut second = SourceCheckProjection::new("fixture/source-owner", 7, VALID, context).unwrap();
    assert_eq!(
        second.project(&request(&first)),
        Err(SourceCheckRefusal::UnknownContext)
    );
    let b = second.project(&request(&second)).unwrap();
    assert_eq!(a.snapshot, b.snapshot);
    assert_ne!(a.context, b.context);
    assert_ne!(a.candidate, b.candidate);
}

#[test]
fn checked_entry_identity_survives_unavailable_expansion_context() {
    let p = profile();
    let startup = p.startup_catalog().unwrap();
    let checked = check_syntax_document(&parse_syntax_document(VALID), &startup).unwrap();
    // Deliberately incomplete expansion context must report its real error,
    // while retaining the checking fact actually established by the reference.
    let ctx = SourceCheckContext::new(
        revision(),
        startup,
        ProfileCatalog::new(),
        CanonicalBackCatalog::new(),
    )
    .unwrap();
    let mut projection = SourceCheckProjection::new("fixture/source-owner", 7, VALID, ctx).unwrap();
    let info = projection.project(&request(&projection)).unwrap();
    assert_eq!(info.check, CheckOutcome::Accepted);
    assert_eq!(
        info.candidate.checked_plot_id.as_deref(),
        Some(checked.plots[0].checked_plot_id.as_str())
    );
    assert_eq!(info.candidate.expanded_plot_id, None);
    assert!(!info.diagnostics.is_empty());
    assert_eq!(info.dependencies, DependencyProvenance::WholeContext);
    assert_eq!(info.plan, RealizationPresence::Absent);
    assert_eq!(info.play, RealizationPresence::Absent);
}
