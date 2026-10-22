//! Workspace checking reuses the same canonical Patchbay sink as installed owners.
use super::catalogs;
use std::fs;

fn patchbay_source() -> String {
    let root = crate::workspace::workspace_root().unwrap();
    fs::read_to_string(root.join("plots/patchbay/main.conduit")).unwrap()
}

#[test]
fn workspace_patchbay_catalog_retains_exact_source_and_canonical_fore() {
    let source = patchbay_source();
    let (startup, profile) = catalogs().unwrap();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let authored =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "patchbay", &profile).unwrap();
    let kind = patchbay_application::patchbay_interaction_contract();
    assert_eq!(authored.expanded.gears.len(), 1);
    assert_eq!(authored.expanded.gears[0].kind_id, kind.kind_id);
    assert_eq!(authored.expanded.gears[0].inputs, kind.inputs);
    assert!(authored.expanded.gears[0].outputs.is_empty());

    let root = crate::workspace::workspace_root().unwrap();
    let output = std::env::temp_dir().join(format!(
        "conduit-patchbay-workspace-catalog-{}.json",
        std::process::id()
    ));
    super::super::bundle_workspace_catalog(&root, &output).unwrap();
    let bytes = fs::read(&output).unwrap();
    fs::remove_file(&output).unwrap();
    let catalog: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let patchbay = catalog["plots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plot| plot["slug"] == "patchbay")
        .unwrap();
    assert_eq!(patchbay["source"], source);
    assert_eq!(
        patchbay["source_document_id"],
        checked.source_document_id.as_str()
    );
    assert_eq!(
        patchbay["checked_plot_id"],
        authored.expanded.checked_plot_id.as_str()
    );
    assert_eq!(
        patchbay["required_kinds"],
        serde_json::json!([patchbay_application::PATCHBAY_INTERACTION_KIND])
    );
}

#[test]
fn workspace_patchbay_catalog_does_not_admit_foreign_kinds_or_wrong_fore_types() {
    let source = patchbay_source();
    let (startup, profile) = catalogs().unwrap();
    let foreign = source.replace(
        patchbay_application::PATCHBAY_INTERACTION_KIND,
        "patchbay/foreign-unregistered-interaction",
    );
    let error = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&foreign),
        &startup,
    )
    .unwrap_err();
    assert_eq!(error.code, "CND-FRM-028");

    let wrong = source.replace("interaction: FaceInteraction", "interaction: Text");
    assert_ne!(wrong, source);
    let refused = match conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&wrong),
        &startup,
    ) {
        Err(_) => true,
        Ok(checked) => {
            conduit_plot::expand_canonical_plot_for_authoring(&checked, "patchbay", &profile)
                .is_err()
        }
    };
    assert!(refused, "a Text Fore cannot become a FaceInteraction sink");
}
