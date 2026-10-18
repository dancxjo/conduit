use super::*;
use conduit_core::{BootId, ConfigurationValue, HostId};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ExpandedAuthoringPlot, PortableExpressionOperation as Op, ProfileCatalog, StartupCatalog,
};

pub(super) fn authored(source: &str) -> ExpandedAuthoringPlot {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    expand_canonical_plot_for_authoring(&checked, "arithmetic", &ProfileCatalog::new()).unwrap()
}
pub(super) fn source(body: &str) -> String {
    format!("plot arithmetic (\n >> value: U64\n result: U64 >>\n) = ({body})\n")
}
fn expression(body: &str) -> ExpandedCanonicalPlot {
    authored(&source(body)).expanded
}
fn program(plot: &ExpandedCanonicalPlot) -> PortableExpressionProgram {
    let ConfigurationValue::Text(encoded) = &plot.gears[0].configuration[0].value else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn owned(host: &StdHost) -> Vec<&CapabilityOffer> {
    host.advertisement()
        .capabilities
        .iter()
        .filter(|offer| owns(offer))
        .collect()
}

#[test]
fn source_batch_is_idempotent_replaces_itself_and_clears_for_an_empty_source() {
    let mut host = StdHost::new();
    let original = host.advertisement().clone();
    let first = expression(". + 1");
    host.prepare_expression_realizations(&first).unwrap();
    let offered = host.advertisement().clone();
    assert_eq!(offered.offer_generation.0, original.offer_generation.0 + 1);
    assert_eq!(owned(&host).len(), 1);
    assert_eq!(
        owned(&host)[0].implementation.implementation_id.as_str(),
        conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION
    );
    host.prepare_expression_realizations(&first).unwrap();
    assert!(
        host.advertisement() == &offered,
        "identical preparation changed the advertisement"
    );
    host.prepare_expression_realizations(&expression(". + 2"))
        .unwrap();
    assert_eq!(owned(&host).len(), 1);
    assert_ne!(owned(&host)[0].kind_id, first.gears[0].kind_id);
    host.prepare_expression_realizations(&authored("plot arithmetic {\n}\n").expanded)
        .unwrap();
    assert!(owned(&host).is_empty());
    // Normalization changes order, not membership or offer facts.
    let initial: std::collections::BTreeMap<_, _> = original
        .capabilities
        .iter()
        .map(|offer| (&offer.capability_id, offer))
        .collect();
    let restored: std::collections::BTreeMap<_, _> = host
        .advertisement()
        .capabilities
        .iter()
        .map(|offer| (&offer.capability_id, offer))
        .collect();
    assert!(
        initial.keys().eq(restored.keys()),
        "existing capability membership changed"
    );
    for (id, offer) in initial {
        assert!(
            restored.get(id) == Some(&offer),
            "existing offer changed: {}",
            id.as_str()
        );
    }
}

#[test]
fn externally_supplied_expression_offers_are_preserved_and_not_replaced() {
    let first = expression(". + 1");
    let custom = conduit_std_offers::pure_expression_std_offer(
        &program(&first),
        first.gears[0].inputs[0].temporal,
    )
    .unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    advertisement.capabilities.push(custom.clone());
    let mut host = StdHost::from_advertisement(advertisement).unwrap();
    host.prepare_expression_realizations(&first).unwrap();
    assert!(owned(&host).is_empty());
    host.prepare_expression_realizations(&expression(". + 2"))
        .unwrap();
    assert_eq!(owned(&host).len(), 1);
    assert!(host.advertisement().capabilities.contains(&custom));
    host.prepare_expression_realizations(&first).unwrap();
    assert!(owned(&host).is_empty());
    assert!(host.advertisement().capabilities.contains(&custom));
}

#[test]
fn a_late_unsupported_native_law_and_foreign_source_leave_the_prior_batch_intact() {
    let mut host = StdHost::new();
    host.prepare_expression_realizations(&expression(". + 9"))
        .unwrap();
    let before = host.advertisement().clone();
    let mixed = authored("type Small = U64 in 0..=10\nplot arithmetic (\n >> wide: U64\n >> small: Small\n result: U64 >>\n narrow: Small >>\n) {\n wide >> (. + 1) >> result\n small >> (.) >> narrow\n}\n");
    let why = host
        .prepare_expression_realizations(&mixed.expanded)
        .unwrap_err();
    assert!(why.contains("native-law-witness-unavailable"), "{why}");
    assert!(
        host.advertisement() == &before,
        "refusal changed the advertisement"
    );
    assert!(host.kernel_resources.is_idle());
    let mut foreign = expression(". + 3");
    foreign.expanded_plot_id = "foreign".into();
    assert!(host
        .prepare_expression_realizations(&foreign)
        .unwrap_err()
        .contains("source identity"));
    assert!(
        host.advertisement() == &before,
        "refusal changed the advertisement"
    );
}

#[test]
fn a_source_exceeding_the_finite_batch_and_generation_exhaustion_never_partially_install() {
    let stages = (1..=33)
        .map(|i| format!("(. + {i})"))
        .collect::<Vec<_>>()
        .join(" >> ");
    let large = authored(&format!("plot arithmetic (\n >> value: U64\n result: U64 >>\n) {{\n value >> {stages} >> result\n}}\n"));
    let mut host = StdHost::new();
    let before = host.advertisement().clone();
    assert!(host
        .prepare_expression_realizations(&large.expanded)
        .unwrap_err()
        .contains("exceeds 32"));
    assert!(
        host.advertisement() == &before,
        "refusal changed the advertisement"
    );
    let mut exhausted = StdHost::new_with_config(crate::StdHostConfig {
        host_id: HostId::from("expressions"),
        boot_id: BootId::from("expressions-boot"),
        offer_generation: OfferGeneration(u64::MAX),
    });
    let before = exhausted.advertisement().clone();
    assert!(exhausted
        .prepare_expression_realizations(&expression(". + 1"))
        .unwrap_err()
        .contains("generation exhausted"));
    assert!(
        exhausted.advertisement() == &before,
        "generation exhaustion changed the advertisement"
    );
}

#[test]
fn real_active_reservations_fence_reconfiguration_until_released() {
    let text = source(". + 1");
    let (mut host, plan, _) =
        crate::host_execution::scalar_tape_fixture::prepare(&text, "arithmetic", true);
    let before = host.advertisement().clone();
    let reservation = host
        .kernel_resources
        .prepare_and_reserve(&before, &plan.fragments[0])
        .unwrap();
    assert!(!host.kernel_resources.is_idle());
    assert!(host
        .prepare_expression_realizations(&expression(". + 2"))
        .unwrap_err()
        .contains("active reservations"));
    assert!(
        host.advertisement() == &before,
        "refusal changed the advertisement"
    );
    host.kernel_resources.release(reservation).unwrap();
    host.prepare_expression_realizations(&expression(". + 2"))
        .unwrap();
    assert!(host.kernel_resources.is_idle());
}

#[test]
fn unretained_proof_hints_are_not_advertised_as_a_safe_reference_profile() {
    let mut program = program(&expression(". + 1"));
    let Op::Binary { proven, .. } = &mut program.root.operation else {
        panic!()
    };
    *proven = true;
    let offer =
        conduit_std_offers::pure_expression_std_offer(&program, conduit_core::PortTemporal::Value)
            .unwrap();
    assert!(
        prepared_reference(&program, offer, "fixture/tape-ineligible@1")
            .unwrap_err()
            .contains("arithmetic-witness-unavailable")
    );
}

#[test]
fn an_eligible_quantity_predicate_does_not_replace_the_filter_kind_with_an_expression() {
    let quantity = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
        conduit_core::DISTANCE_INFO_ID,
    ))
    .unwrap();
    let boolean =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID))
            .unwrap();
    let input = || conduit_plot::PortableExpressionNode {
        value_type: quantity.clone(),
        operation: Op::Input,
    };
    let program = PortableExpressionProgram {
        input_type: quantity.clone(),
        input_maximum_bytes: None,
        output_type: boolean.clone(),
        root: conduit_plot::PortableExpressionNode {
            value_type: boolean,
            operation: Op::Binary {
                operator: conduit_plot::BinaryOperator::Equal,
                proven: false,
                left: Box::new(input()),
                right: Box::new(input()),
            },
        },
    };
    assert!(conduit_std_offers::pure_exact_quantity_std_offer(
        &program,
        conduit_core::PortTemporal::Value
    )
    .is_ok());
    let baseline =
        conduit_std_offers::pure_filter_std_offer(&program, conduit_core::PortTemporal::Value)
            .unwrap();
    let selected =
        prepared_reference(&program, baseline.clone(), "scalar-tape/filter-contract@1").unwrap();
    assert_eq!(selected.kind_id, baseline.kind_id);
    assert_eq!(selected.semantic_contract, baseline.semantic_contract);
    assert_eq!(selected.implementation, baseline.implementation);
}
