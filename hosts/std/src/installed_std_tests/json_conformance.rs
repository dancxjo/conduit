use super::*;

const FORM: &str = "form 0\n\njson_round_trip {\n source: conduit.test/json-text-source\n decode: json/decode\n encode: json/encode\n sink: conduit.test/json-text-sink\n source.value -> decode.value\n decode.value -> encode.value\n encode.value -> sink.value\n}\n";

#[test]
fn ordinary_form_runs_shared_bounded_json_through_the_production_kernel() {
    let form = parse(FORM, &installed_std::test_catalog()).expect("JSON codec Form parses");
    let config = StdHostConfig {
        host_id: HostId::from("json-host"),
        boot_id: BootId::from("json-boot"),
        offer_generation: OfferGeneration(1),
    };
    let mut host = StdHost::new_with_composition(config, crate::StdHostComposition::reference());
    let plan = host.plan_local(&form, None).expect("JSON codec Form plans");
    let fragment = plan.fragments[0].clone();
    for kind in [
        conduit_std_catalog::JSON_DECODE_KIND,
        conduit_std_catalog::JSON_ENCODE_KIND,
    ] {
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .expect("JSON operation is placed");
        assert!(placement.authority.is_empty());
        assert!(placement.resources.is_empty());
        assert_eq!(placement.host_operations.len(), 1);
        assert_eq!(
            placement.host_operations[0].maximum_input_bytes,
            conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32
        );
    }

    let mut output = Vec::new();
    let report = host
        .run_fragment_to(
            fragment,
            &mut output,
            &mut RecordingTimer { waits: Vec::new() },
        )
        .expect("JSON codec Form runs through the production kernel");
    assert!(String::from_utf8(output)
        .unwrap()
        .lines()
        .any(|line| line == "{\"a\":\"ok\",\"z\":1.23}"));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn json_is_an_explicit_composition_family() {
    let config = |name: &str| StdHostConfig {
        host_id: HostId::from(name),
        boot_id: BootId::from(format!("{name}-boot")),
        offer_generation: OfferGeneration(1),
    };
    let absent =
        StdHost::new_with_composition(config("absent"), crate::StdHostComposition::minimal());
    let present = StdHost::new_with_composition(
        config("present"),
        crate::StdHostComposition::minimal().with_json(),
    );
    assert!(!absent
        .advertisement()
        .capabilities
        .iter()
        .any(|offer| offer.kind_id.as_str().starts_with("json/")));
    assert_eq!(
        present
            .advertisement()
            .capabilities
            .iter()
            .filter(|offer| offer.kind_id.as_str().starts_with("json/"))
            .count(),
        2
    );
}
