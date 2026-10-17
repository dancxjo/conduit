//! Reviewed dimensional arithmetic through ordinary installed Source/Plan/Play.
use conduit_core::*;
#[path = "common/angle_host.rs"]
mod angle_host;
const SOURCE: &str = include_str!("../../../plots/physical-values/triad.conduit");
#[test]
fn reviewed_length_quotient_and_product_keep_derived_meaning_through_installed_play() {
    let length = Quantity::parse_plot_literal("1m").unwrap().dimension();
    let time = Quantity::parse_plot_literal("1s").unwrap().dimension();
    for (entry, dimension, coefficient, exponent) in [
        ("speed", length.quotient(time).unwrap(), 15, -2),
        ("area", length.product(length).unwrap(), 6, -1),
    ] {
        let (mut host, plan) = angle_host::prepare(SOURCE, entry, "distance");
        assert_eq!(
            plan.source_document_id,
            conduit_plot::parse_syntax_document(SOURCE).source_document_id()
        );
        let output = angle_host::run(
            &mut host,
            &plan,
            "distance",
            vec![Quantity::parse_plot_literal("30cm")
                .unwrap()
                .encode()
                .to_vec()],
        );
        let value = Quantity::decode(&output[0]).unwrap();
        assert_eq!(value.dimension(), dimension);
        assert_eq!(
            (value.coefficient(), value.exponent()),
            (coefficient, exponent)
        );
        assert_eq!(value.role(), QuantityRole::Linear);
        assert_eq!(
            value.unit().exact_scale(),
            DefinitionScalar::new(1, 1, 0).unwrap()
        );
    }
}
