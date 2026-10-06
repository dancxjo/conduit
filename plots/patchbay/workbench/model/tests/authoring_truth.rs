use conduit_core::{ConfigurationValue, KindId};
use conduit_patchbay_workbench::{PlotEditor, PlotEditorError};

const SOURCE: &str = r#"plot authoring {
    source: text/literal("catalog")
    join: text/join(prefix = " ")
    upper: text/upper
    delay: time/debounce(duration-ms = 25ms, policy = "trailing", maximum-values = 4)
    display: presentation/text
    source >> join >> upper >> display
}
"#;

#[test]
fn unchanged_revision_and_ports_do_not_hide_configuration_drift() {
    let original = conduit_semantic_catalog::standard_profile_catalog();
    let kind = original.canonical_kind(&KindId::from("text/join")).unwrap();
    for changed_default in [false, true] {
        let mut divergent = kind.clone();
        if changed_default {
            divergent.configuration[0].default_value = ConfigurationValue::Text("drift".into());
        } else {
            divergent.configuration[0].rule =
                conduit_core::KindConfigurationRule::TextBytes { maximum: 1 };
        }
        assert_eq!(
            divergent.kind_contract_revision,
            kind.kind_contract_revision
        );
        assert_eq!(divergent.inputs, kind.inputs);
        assert_eq!(divergent.outputs, kind.outputs);
        let mut profile = conduit_plot::ProfileCatalog::new();
        profile.insert_kind(divergent).unwrap();
        let startup = profile.startup_catalog().unwrap();
        let editor = PlotEditor::from_source_with_catalogs(
            "drift.conduit".into(),
            "plot drift {\n}\n".into(),
            startup,
            profile,
        )
        .unwrap();
        let inventory = editor.authoring_catalog().unwrap();
        assert!(
            !inventory
                .iter()
                .find(|entry| entry.contract.kind_id == kind.kind_id)
                .unwrap()
                .authorable
        );
    }
}

#[test]
fn reroute_accepts_the_same_finite_to_standing_flow_contract_as_connect() {
    let source = "plot main {\n clock: time/tick(count = 1, period-ms = 0)\n sink: presentation/tick\n alternate: presentation/tick\n clock.tick >> sink.tick\n}\n";
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_time::install_tick_catalog(&mut startup, &mut profile).unwrap();
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)
        .unwrap();
    let mut editor = PlotEditor::from_source_with_catalogs(
        "reroute.conduit".into(),
        source.into(),
        startup,
        profile,
    )
    .unwrap();
    let graph = editor.patchbay_graph_for_authoring("main").unwrap();
    let upper = graph
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "time/tick")
        .unwrap();
    let alternate = graph
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "main/alternate")
        .unwrap();
    assert_ne!(
        upper.outputs[0].descriptor.temporal,
        alternate.inputs[0].descriptor.temporal
    );
    let cord = graph
        .cords
        .iter()
        .find(|cord| cord.source_port == upper.outputs[0].identity)
        .unwrap();
    editor
        .clone()
        .connect_ports(
            0,
            &graph.expanded_plot_id,
            &upper.outputs[0].identity,
            &alternate.inputs[0].identity,
        )
        .unwrap();
    editor
        .reroute_cord_endpoint(
            0,
            &graph.expanded_plot_id,
            &cord.identity,
            &alternate.inputs[0].identity,
        )
        .unwrap();
    let after = editor.patchbay_graph_for_authoring("main").unwrap();
    assert_eq!(after.cords.len(), graph.cords.len());
    assert!(after
        .cords
        .iter()
        .any(|cord| cord.source_port == upper.outputs[0].identity
            && cord.sink_port == alternate.inputs[0].identity));
}

#[test]
fn catalog_and_configuration_queries_preserve_exact_checked_source() {
    let editor = PlotEditor::from_source("authoring.conduit".into(), SOURCE.into()).unwrap();
    let before = editor.view();
    let graph = editor.patchbay_graph_for_authoring("authoring").unwrap();
    assert_eq!(graph.gears.len(), 5);
    let catalog = editor.authoring_catalog().unwrap();
    let debounce = catalog
        .iter()
        .find(|kind| kind.contract.kind_id.as_str() == "time/debounce")
        .unwrap();
    assert!(debounce.authorable);
    assert_eq!(debounce.contract.configuration.len(), 3);
    assert_eq!(debounce.startup_parameters.len(), 3);
    assert!(editor
        .validate_authoring_configuration(
            0,
            &graph.expanded_plot_id,
            "delay",
            "maximum-values",
            ConfigurationValue::U64(0)
        )
        .is_err());
    editor
        .validate_authoring_configuration(
            0,
            &graph.expanded_plot_id,
            "delay",
            "maximum-values",
            ConfigurationValue::U64(3),
        )
        .unwrap();
    assert_eq!(editor.view(), before);
}

#[test]
fn compatibility_queries_and_committed_edits_have_identical_outcomes() {
    let editor = PlotEditor::from_source("authoring.conduit".into(), SOURCE.into()).unwrap();
    let graph = editor.patchbay_graph_for_authoring("authoring").unwrap();
    let source = &graph.gears[0].outputs[0].identity;
    let before = editor.view();
    let candidates = editor
        .authoring_connections(0, &graph.expanded_plot_id, source)
        .unwrap();
    assert!(!candidates.is_empty());
    for candidate in candidates {
        let mut copy = editor.clone();
        let result =
            copy.connect_ports(0, &graph.expanded_plot_id, source, &candidate.sink_identity);
        assert_eq!(candidate.compatible, result.is_ok());
        assert_eq!(
            candidate.diagnostic,
            result.err().map(|error| error.to_string())
        );
    }
    assert_eq!(editor.view(), before);
    assert!(matches!(
        editor.authoring_connections(1, &graph.expanded_plot_id, source),
        Err(PlotEditorError::StaleRevision { .. })
    ));
}

#[test]
fn selected_catalog_is_not_replaced_by_default_catalog_during_an_edit() {
    let mut editor = PlotEditor::from_source_with_catalogs(
        "restricted.conduit".into(),
        "plot restricted {\n}\n".into(),
        conduit_plot::StartupCatalog::new(),
        conduit_plot::ProfileCatalog::new(),
    )
    .unwrap();
    let before = editor.view();
    let inventory = editor.authoring_catalog().unwrap();
    assert!(
        !inventory.is_empty(),
        "catalog existence does not depend on offered Backs"
    );
    assert!(inventory.iter().all(|kind| !kind.authorable));
    assert!(editor
        .place_palette_kind(0, &KindId::from("text/literal"))
        .is_err());
    assert_eq!(editor.view(), before);
}

#[test]
fn semantic_rename_and_delete_report_orphans_without_remapping_layouts() {
    use patchbay_application::{PatchbayWorkspace, WorkspaceBasis, WorkspaceError};
    let mut editor = PlotEditor::from_source("authoring.conduit".into(), SOURCE.into()).unwrap();
    let before = editor.patchbay_graph_for_authoring("authoring").unwrap();
    let join = before
        .gears
        .iter()
        .find(|gear| gear.gear_id.as_str() == "authoring/join")
        .unwrap();
    let basis = WorkspaceBasis {
        source_document_id: before.source_document_id.as_str().into(),
        checked_plot_id: before.checked_plot_id.as_str().into(),
    };
    let workspace: PatchbayWorkspace = serde_json::from_value(serde_json::json!({
        "schema": patchbay_application::PATCHBAY_WORKSPACE_SCHEMA,
        "basis": basis,
        "active_layout": "Teaching",
        "layouts": [{
            "name": "Teaching",
            "positions": [{"subject": join.identity, "x": 20, "y": 30}],
            "routes": [], "frames": [], "notes": [], "collapsed": [],
            "viewport": {"x": 0, "y": 0, "zoom": 1},
            "lens": "plan"
        }]
    }))
    .unwrap();
    workspace.validate().unwrap();
    let retained = workspace.clone();
    let renamed = SOURCE
        .replace("join: text/join", "renamed: text/join")
        .replace("source >> join >>", "source >> renamed >>");
    editor.replace_source(renamed).unwrap();
    editor.recheck().unwrap();
    let after = editor.patchbay_graph_for_authoring("authoring").unwrap();
    let changed = WorkspaceBasis {
        source_document_id: after.source_document_id.as_str().into(),
        checked_plot_id: after.checked_plot_id.as_str().into(),
    };
    let subjects = after
        .subject_identities()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let correlation = workspace.correlate(&changed, &subjects).unwrap();
    assert!(!correlation.basis_matches);
    assert_eq!(correlation.orphaned_subjects, vec![join.identity.clone()]);
    assert_eq!(
        workspace.layout_for_basis(&changed),
        Err(WorkspaceError::ChangedBasis)
    );
    editor
        .remove_gear(editor.view().revision, "renamed")
        .unwrap();
    let deleted = editor.patchbay_graph_for_authoring("authoring").unwrap();
    let subjects = deleted
        .subject_identities()
        .map(str::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        workspace
            .correlate(&changed, &subjects)
            .unwrap()
            .orphaned_subjects,
        vec![join.identity.clone()]
    );
    assert_eq!(
        workspace, retained,
        "semantic migration must not fabricate geometry"
    );
}
