//! Public pure semantic callers retain their exact owner Fore and Source Back.
use conduit_plot::*;
use conduit_semantic_catalog::{
    install_acoustic_control_backs, install_acoustic_control_catalogs,
    prepare_acoustic_control_source_package, ACOUSTIC_CONTROL_COMPONENTS,
};

#[test]
fn public_clock_and_control_callers_expand_exact_resource_free_owner_source() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_acoustic_control_catalogs(&mut startup, &mut profiles).unwrap();
    let original_profiles = profiles.clone();
    let package = prepare_acoustic_control_source_package().unwrap();
    let mut backs = CanonicalBackCatalog::new();
    install_acoustic_control_backs(&profiles, &mut backs).unwrap();
    assert_eq!(profiles, original_profiles);
    for (name, input, output, in_port, out_port) in [
        (
            "audio/clock-binding-valid",
            "AudioClockBinding",
            "Boolean",
            "binding",
            "valid",
        ),
        (
            "audio/clock-query-eligible",
            "AudioClockProjectionRequest",
            "Boolean",
            "request",
            "eligible",
        ),
        (
            "audio/clock-project",
            "AudioClockProjectionEligible",
            "AudioClockProjectionRaw",
            "value",
            "projected",
        ),
        (
            "audio/control-query-time",
            "AudioControlProjectionPoint",
            "AudioTrajectoryQuery",
            "value",
            "result",
        ),
        (
            "audio/control-final-time",
            "AudioControlProjectionSchedule",
            "AudioTrajectoryQuery",
            "value",
            "result",
        ),
    ] {
        let kind = profiles.canonical_kind(&name.into()).unwrap();
        let owner = package
            .checked
            .plots
            .iter()
            .find(|plot| plot.name == name)
            .unwrap();
        assert_eq!(kind.checked_front(), owner.checked_front());
        assert!(kind.checked_front().resource_ports().is_empty());
        let back = backs.checked_definition(&kind.kind_id).unwrap();
        assert_eq!(
            back.realization.source_document_id,
            package.checked.source_document_id
        );
        assert_eq!(back.realization.checked_plot_id, owner.checked_plot_id);
        assert_eq!(
            back.realization.kind_contract_revision,
            kind.kind_contract_revision
        );
        let source = format!("plot caller (\n >> input: {input}\n output: {output} >>\n) {{\n component: {name}()\n input >> component.{in_port}\n component.{out_port} >> output\n}}\n");
        let checked = check_syntax_document(&parse_syntax_document(&source), &startup)
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        let expanded =
            expand_canonical_plot_for_authoring_with_backs(&checked, "caller", &profiles, &backs)
                .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert!(!expanded.expanded.gears.is_empty());
    }
}

#[test]
fn missing_or_changed_contract_refuses_without_partially_installing_backs() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_acoustic_control_catalogs(&mut startup, &mut profiles).unwrap();
    let mut changed = ProfileCatalog::new();
    for (index, name) in ACOUSTIC_CONTROL_COMPONENTS.iter().enumerate() {
        let mut kind = profiles.canonical_kind(&(*name).into()).unwrap().clone();
        if index + 1 == ACOUSTIC_CONTROL_COMPONENTS.len() {
            kind.kind_contract_revision = "unreviewed/control-source@0".into();
        }
        changed.insert_kind(kind).unwrap();
    }
    let mut backs = CanonicalBackCatalog::new();
    let before = backs.clone();
    assert!(install_acoustic_control_backs(&changed, &mut backs).is_err());
    assert_eq!(backs, before);
    assert!(install_acoustic_control_backs(&ProfileCatalog::new(), &mut backs).is_err());
    assert_eq!(backs, before);
    install_acoustic_control_backs(&profiles, &mut backs).unwrap();
    let installed = backs.clone();
    assert!(install_acoustic_control_backs(&profiles, &mut backs).is_err());
    assert_eq!(backs, installed);
}

#[test]
fn wrong_owner_input_type_refuses_public_caller_admission() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_acoustic_control_catalogs(&mut startup, &mut profiles).unwrap();
    let source = "plot wrong (\n >> input: AudioTrajectoryQuery\n valid: Boolean >>\n) {\n component: audio/clock-binding-valid()\n input >> component.binding\n component.valid >> valid\n}\n";
    assert!(check_syntax_document(&parse_syntax_document(source), &startup).is_err());
}
