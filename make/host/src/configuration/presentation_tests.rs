use super::*;
use crate::{PrerequisiteNode as Node, PresenterMetadata};

const TARGET: &str = "conduitos/aarch64/virt";
fn catalog() -> MakeCatalog {
    let mut catalog = MakeCatalog::canonical();
    for (id, prerequisites) in [
        ("serial", vec![Node::Base("serial/text".into())]),
        (
            "graphics",
            vec![
                Node::Base("display/scanout".into()),
                Node::Resource("face/surface".into()),
                Node::Facility("compositor/native@1".into()),
                Node::HostCall("present".into()),
            ],
        ),
    ] {
        catalog.presenters.insert(
            id.into(),
            PresenterMetadata {
                targets: vec![TARGET.into()],
                prerequisites,
            },
        );
    }
    catalog.dependencies.insert(
        Node::Base("display/scanout".into()),
        vec![Node::Driver("framebuffer".into())],
    );
    catalog
}
fn default() -> TargetPresenter {
    TargetPresenter {
        id: "serial/main".into(),
        implementation_id: "serial".into(),
        interactive: false,
    }
}
fn surface() -> ResourceBudget {
    ResourceBudget {
        id: "surface/main".into(),
        class: "face/surface".into(),
        slots: 1,
        bytes: 1024,
    }
}
fn bases() -> Vec<(String, String)> {
    vec![("display/scanout".into(), "framebuffer".into())]
}

#[test]
fn selected_serial_default_preserves_identity_and_noninteractive_contract() {
    let r = resolve(
        Some(&default()),
        &["present".into()],
        TARGET,
        &catalog(),
        &[("serial/text".into(), "uart".into())],
        &[],
    )
    .unwrap();
    let p = r.presenter.unwrap();
    assert_eq!(p.id, "serial/main");
    assert_eq!(p.implementation, "serial");
    assert!(!p.interactive);
    assert_eq!(r.host_calls, ["present"]);
    assert!(r.facilities.is_empty());
}
#[test]
fn selected_scanout_and_surface_choose_alternative_to_serial_default() {
    let r = resolve(
        Some(&default()),
        &[],
        TARGET,
        &catalog(),
        &bases(),
        &[surface()],
    )
    .unwrap();
    assert_eq!(r.presenter.unwrap().implementation, "graphics");
    assert_eq!(r.host_calls, ["present"]);
    assert_eq!(r.facilities, ["compositor/native@1"]);
}
#[test]
fn selected_default_cannot_hide_a_second_eligible_realization() {
    let mut bases = bases();
    bases.push(("serial/text".into(), "uart".into()));
    let result = resolve(
        Some(&default()),
        &[],
        TARGET,
        &catalog(),
        &bases,
        &[surface()],
    );
    assert!(
        matches!(result, Err(errors) if errors == vec![ConfigurationDiagnostic::AmbiguousFaceRealization { implementations: vec!["graphics".into(), "serial".into()] }])
    );
}
#[test]
fn missing_or_mismatched_roots_do_not_derive_graphics() {
    for (bases, resources, target) in [
        (bases(), vec![], TARGET),
        (
            vec![("display/scanout".into(), "wrong-driver".into())],
            vec![surface()],
            TARGET,
        ),
        (bases(), vec![surface()], "conduitos/riscv64/virt"),
    ] {
        let r = resolve(None, &[], target, &catalog(), &bases, &resources).unwrap();
        assert!(r.presenter.is_none());
        assert!(r.host_calls.is_empty());
        assert!(r.facilities.is_empty());
    }
}
#[test]
fn legacy_default_without_metadata_keeps_its_existing_selection() {
    let legacy = TargetPresenter {
        implementation_id: "legacy".into(),
        ..default()
    };
    let r = resolve(
        Some(&legacy),
        &[],
        TARGET,
        &catalog(),
        &bases(),
        &[surface()],
    )
    .unwrap();
    assert_eq!(r.presenter.unwrap().implementation, "legacy");
}
