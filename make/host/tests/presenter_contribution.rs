use conduit_host_make::{
    MakeCatalog, PackageCatalogContribution, PrerequisiteNode, PresenterMetadata,
};
use std::collections::BTreeMap;

fn contribution(target: &str) -> PackageCatalogContribution {
    PackageCatalogContribution {
        presenters: BTreeMap::from([(
            "presenter/shared-serial@1".into(),
            PresenterMetadata {
                targets: vec![target.into()],
                prerequisites: vec![PrerequisiteNode::Base("serial/text".into())],
            },
        )]),
        ..Default::default()
    }
}

#[test]
fn adding_a_target_to_one_presenter_contract_keeps_existing_targets_in_either_order() {
    let first = contribution("conduitos/aarch64/virt");
    let second = contribution("conduitos/armv6/raspberry-pi-model-b-plus-v1.2");
    let a = MakeCatalog::canonical()
        .with_catalog_contribution(&first)
        .with_catalog_contribution(&second);
    let b = MakeCatalog::canonical()
        .with_catalog_contribution(&second)
        .with_catalog_contribution(&first);
    assert_eq!(
        a.presenters["presenter/shared-serial@1"],
        b.presenters["presenter/shared-serial@1"]
    );
    assert_eq!(a.presenters["presenter/shared-serial@1"].targets.len(), 2);
    let repeated = a.with_catalog_contribution(&first);
    assert_eq!(
        repeated.presenters["presenter/shared-serial@1"]
            .targets
            .len(),
        2
    );
}
