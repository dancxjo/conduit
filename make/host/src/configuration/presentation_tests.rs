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
fn selected_fixed_anchor_and_one_resource_backed_realization_are_both_retained() {
    let mut bases = bases();
    bases.push(("serial/text".into(), "uart".into()));
    let result = resolve(
        Some(&default()),
        &[],
        TARGET,
        &catalog(),
        &bases,
        &[surface()],
    )
    .unwrap();
    let graphical = result.presenter.unwrap();
    assert_eq!(graphical.implementation, "graphics");
    assert_ne!(graphical.id, result.supplemental[0].id);
    assert_eq!(result.supplemental[0].implementation, "serial");
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
fn legacy_anchor_is_retained_with_selected_graphics() {
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
    assert_eq!(r.presenter.unwrap().implementation, "graphics");
    assert_eq!(r.supplemental[0].implementation, "legacy");
}

#[test]
fn fixed_anchor_does_not_hide_two_optional_realizations() {
    let mut catalog = catalog();
    let graphics = catalog.presenters["graphics"].clone();
    catalog.presenters.insert("other-graphics".into(), graphics);
    let mut bases = bases();
    bases.push(("serial/text".into(), "uart".into()));
    let result = resolve(
        Some(&default()),
        &[],
        TARGET,
        &catalog,
        &bases,
        &[surface()],
    );
    assert!(matches!(result, Err(errors) if errors == vec![
        ConfigurationDiagnostic::AmbiguousFaceRealization {
            implementations: vec!["graphics".into(), "other-graphics".into()]
        }
    ]));
}

#[test]
fn selected_metadata_anchor_retains_its_own_facility_and_host_call_closure() {
    let graphical = TargetPresenter {
        id: "native/main".into(),
        implementation_id: "graphics".into(),
        interactive: true,
    };
    let result = resolve(
        Some(&graphical),
        &[],
        TARGET,
        &catalog(),
        &bases(),
        &[surface()],
    )
    .unwrap();
    assert_eq!(result.presenter.unwrap().implementation, "graphics");
    assert!(result.supplemental.is_empty());
    assert_eq!(result.host_calls, ["present"]);
    assert_eq!(result.facilities, ["compositor/native@1"]);
}
