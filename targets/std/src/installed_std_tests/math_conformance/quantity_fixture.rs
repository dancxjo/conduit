use super::{host, installed_std};

pub(super) fn prepare(
    source: &str,
    entry: &str,
) -> (
    conduit_plot::ExpandedCanonicalPlot,
    conduit_core::HostAdvertisement,
) {
    let mut catalog = installed_std::test_catalog();
    catalog
        .insert_kind(
            conduit_semantic_catalog::scalar_literal_contract().into_semantic_contract(
                conduit_semantic_catalog::VALUE_PRIMITIVE_CONTRACT_REVISION,
            ),
        )
        .unwrap();
    let value_type = conduit_semantic_catalog::wrapped_quantity_type();
    catalog
        .insert_kind(conduit_semantic_catalog::structured_face_semantic_contract(
            "Quantity",
            &value_type,
        ))
        .unwrap();
    let syntax = conduit_plot::parse_syntax_document(source);
    let mut startup = catalog.startup_catalog().unwrap();
    startup
        .insert_value_kind_alias(
            "Scalar",
            conduit_core::kind_id(conduit_core::SCALAR_INFO_ID),
        )
        .unwrap();
    startup
        .insert_value_kind_alias(
            "Quantity",
            conduit_core::kind_id(conduit_core::QUANTITY_INFO_ID),
        )
        .unwrap();
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let plot = conduit_plot::expand_canonical_plot(&checked, entry, &catalog).unwrap();
    let mut advertisement = host("quantity-output-host").advertisement().clone();
    advertisement
        .capabilities
        .push(conduit_std_offers::structured_face_std_offer(
            "Quantity",
            &value_type,
        ));
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    (plot, advertisement)
}
