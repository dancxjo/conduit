//! Ordinary checked quantities cross the installed browser Plan and kernel.
use super::*;
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{HostedValueStore, ValueStorage};

fn source(kind: &str, left: &str, right: &str, predicate: &str) -> String {
    let names = if kind.contains("compare") {
        ["left", "right"]
    } else {
        ["source", "to"]
    };
    format!(
        r#"plot checked-quantity {{
 operation: {kind}({} = {left}, {} = {right})
 show: presentation/bool-value
 operation.receipt >> ({predicate}) >> show.value
}}."#,
        names[0], names[1]
    )
}
fn converted(coefficient: i128, exponent: i16) -> String {
    format!("variant/is(.result, \"converted\") ? (.result.converted.coefficient == {coefficient} ? .result.converted.exponent == {exponent} : false) : false")
}
fn run(source: &str) -> (TourSession, TourEffect) {
    let (session, effect) =
        TourSession::prepare("browser/exact-quantity", "boot/exact-quantity", source, 1)
            .unwrap_or_else(|error| panic!("{error}\n{source}"));
    let TourHostEffect::Manifestation(effect) = effect else {
        panic!("planned Boolean effect")
    };
    assert_eq!(effect.text.as_deref(), Some("true"));
    assert_eq!(effect.presentation_kind, "presentation/bool-value");
    assert_eq!(effect.plan_id, session.fragments[0].plan_id.as_str());
    assert!(effect
        .expanded_gears
        .iter()
        .any(|gear| gear.kind_id.starts_with("units/")));
    (session, *effect)
}
fn finish(mut session: TourSession, effect: TourEffect) {
    let capacity = session.scheduler.values().allocation_capacities();
    let TourProgress::Receipt(receipt) = session.advance().unwrap() else {
        panic!("completed receipt")
    };
    assert_eq!(receipt.disposition, "completed");
    assert_eq!(receipt.active_play_id, effect.active_play_id);
    assert_eq!(session.scheduler.values().allocation_capacities(), capacity);
}

#[test]
fn exact_quantity_browser_all_prefixes_and_affine_results_execute_checked_receipts() {
    for (prefix, exponent) in [
        ("da", 1),
        ("h", 2),
        ("k", 3),
        ("M", 6),
        ("G", 9),
        ("T", 12),
        ("P", 15),
        ("E", 18),
        ("Z", 21),
        ("Y", 24),
        ("R", 27),
        ("Q", 30),
        ("d", -1),
        ("c", -2),
        ("m", -3),
        ("µ", -6),
        ("n", -9),
        ("p", -12),
        ("f", -15),
        ("a", -18),
        ("z", -21),
        ("y", -24),
        ("r", -27),
        ("q", -30),
    ] {
        let source = source(
            "units/convert",
            &format!("1{prefix}m"),
            "m",
            &converted(1, exponent),
        );
        let (session, effect) = run(&source);
        finish(session, effect);
    }
    for (value, target, coefficient, exponent) in [
        ("1kHz", "Hz", 1, 3),
        ("1µs", "ns", 1, 3),
        ("1cm²", "mm²", 1, 2),
        ("0°C", "K", 27315, -2),
        ("30°C", "°F", 86, 0),
        ("1Qm", "qm", 1, 60),
        ("1qm", "Qm", 1, -60),
        ("1MB", "B", 1, 6),
        ("1MiB", "B", 1048576, 0),
    ] {
        let (session, effect) = run(&source(
            "units/convert",
            value,
            target,
            &converted(coefficient, exponent),
        ));
        finish(session, effect);
    }
}

#[test]
fn exact_quantity_browser_semantic_roles_and_refusals_execute_through_the_kernel() {
    for (kind, left, right, predicate) in [
        ("units/convert-temperature-difference", "TemperatureDelta(9, °F)", "K", converted(5, 0)),
        ("units/convert-temperature-difference", "TemperatureDelta(1, m°C)", "K", converted(1, -3)),
        ("units/compare", "1000mm", "0.001km", "variant/is(.result, \"equal\")".into()),
        ("units/compare", "1°F", "0°C", "variant/is(.result, \"less\")".into()),
        ("units/compare", "1MB", "1MiB", "variant/is(.result, \"less\")".into()),
        ("units/compare-temperature-differences", "TemperatureDelta(9, °F)", "TemperatureDelta(5, K)", "variant/is(.result, \"equal\")".into()),
        ("units/convert", "1°F", "°C", "variant/is(.result, \"refused\") ? .result.refused == \"inexact\" : false".into()),
        ("units/convert", "1Hz", "m", "variant/is(.result, \"refused\") ? .result.refused == \"incompatible-dimensions\" : false".into()),
        ("units/convert", "1Qm³", "qm³", "variant/is(.result, \"refused\") ? .result.refused == \"overflow\" : false".into()),
        ("units/compare", "1m", "1s", "variant/is(.result, \"refused\") ? .result.refused == \"incompatible-dimensions\" : false".into()),
    ] {
        let (session, effect) = run(&source(kind, left, right, &predicate));
        finish(session, effect);
    }
}

fn preparation_refuses(placement: &PlannedGear) {
    let installation = crate::installed_browser::factory(&placement.implementation_id).unwrap();
    let mut values = HostedValueStore::new(4, 8192, 32768).unwrap();
    let before = values.allocation_capacities();
    assert!((installation.prepare)(placement, &mut values).is_err());
    assert_eq!(values.allocation_capacities(), before);
}

#[test]
fn exact_quantity_browser_admission_refuses_identity_and_configuration_drift() {
    for (kind, left, right, predicate) in [
        ("units/convert", "1kHz", "Hz", converted(1, 3)),
        (
            "units/convert-temperature-difference",
            "TemperatureDelta(9, °F)",
            "K",
            converted(5, 0),
        ),
        (
            "units/compare",
            "1m",
            "1000mm",
            "variant/is(.result, \"equal\")".into(),
        ),
        (
            "units/compare-temperature-differences",
            "TemperatureDelta(9, °F)",
            "TemperatureDelta(5, K)",
            "variant/is(.result, \"equal\")".into(),
        ),
    ] {
        let (session, _) = run(&source(kind, left, right, &predicate));
        let placement = session.fragments[0]
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == kind)
            .unwrap();
        let mut changed = placement.clone();
        changed.artifact_id = "wrong/browser-quantity".into();
        preparation_refuses(&changed);
        let mut changed = placement.clone();
        changed.capability_id = "wrong/capability".into();
        preparation_refuses(&changed);
        let mut changed = placement.clone();
        changed.kind_contract_revision = "wrong/revision".into();
        preparation_refuses(&changed);
        let mut changed = placement.clone();
        changed.semantic_contract.laws.clear();
        preparation_refuses(&changed);
        let mut changed = placement.clone();
        changed.outputs[0].value_kind = "wrong/receipt".into();
        preparation_refuses(&changed);
        let mut changed = placement.clone();
        changed.configuration.push(changed.configuration[0].clone());
        preparation_refuses(&changed);
        let mut changed = placement.clone();
        changed.configuration[0].value = ConfigurationValue::Text("1".repeat(129));
        preparation_refuses(&changed);
    }
    for (kind, left, right) in [
        ("units/compare", "21C", "1°C"),
        ("units/convert-temperature-difference", "1Hz", "K"),
    ] {
        assert!(
            TourSession::prepare(
                "browser/refusal",
                "boot/refusal",
                &source(kind, left, right, "true"),
                2
            )
            .is_err(),
            "invalid typed quantity source refuses"
        );
    }
}

#[test]
fn exact_quantity_browser_stored_receipts_survive_pressure_byte_for_byte() {
    use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
    use conduit_kernel::PortId;
    const PORTS: usize = crate::installed_browser::BROWSER_PORTS_PER_GEAR;
    for (kind, left, right, predicate) in [
        ("units/convert", "1Qm", "qm", converted(1, 60)),
        (
            "units/convert-temperature-difference",
            "TemperatureDelta(9, °F)",
            "K",
            converted(5, 0),
        ),
        (
            "units/compare",
            "1000mm",
            "0.001km",
            "variant/is(.result, \"equal\")".into(),
        ),
        (
            "units/compare-temperature-differences",
            "TemperatureDelta(9, °F)",
            "TemperatureDelta(5, K)",
            "variant/is(.result, \"equal\")".into(),
        ),
    ] {
        let (session, _) = run(&source(kind, left, right, &predicate));
        let placement = session.fragments[0]
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == kind)
            .unwrap();
        let expected = conduit_plot::quantity_conversion::prepare_operation_configuration(
            kind,
            &placement.configuration,
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let installation = crate::installed_browser::factory(&placement.implementation_id).unwrap();
        let mut values = HostedValueStore::new(4, 8192, 32768).unwrap();
        let mut back = (installation.prepare)(placement, &mut values).unwrap();
        let capacity = values.allocation_capacities();
        let input = StepInputBytes::test_frame([None; PORTS], None);
        for _ in 0..1000 {
            let mut blocked =
                StepIo::test_frame([None; PORTS], [false; PORTS], [None; PORTS], None, 8);
            assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
            assert!(blocked.test_output(PortId(0)).is_none());
        }
        let mut ready =
            StepIo::test_frame([None; PORTS], [false; PORTS], [Some(4096); PORTS], None, 8);
        assert_eq!(back.step(&mut ready, &input), StepOutcome::Complete);
        assert_eq!(
            values.get(ready.test_output(PortId(0)).unwrap()).unwrap(),
            expected
        );
        assert_eq!(values.allocation_capacities(), capacity);
    }
}

#[test]
fn full_name_receipt_comparator_runs_through_browser_kernel() {
    for (source, target, expected, result) in [
        ("1kHz", "Hz", "1000Hz", "true"),
        ("1kHz", "Hz", "999Hz", "false"),
        ("1Hz", "m", "1m", "false"),
        ("1kHz", "Hz", "1m", "false"),
    ] {
        let authored = format!(
            r#"plot converted-equals-demo {{
 operation: units/convert(source = {source}, to = {target})
 exact: units/converted-equals(expected = {expected})
 show: presentation/bool-value
 operation.receipt >> exact.receipt
 exact.result >> show.value
}}."#
        );
        let (session, effect) =
            TourSession::prepare("browser/comparator", "boot/comparator", &authored, 1).unwrap();
        let TourHostEffect::Manifestation(effect) = effect else {
            panic!("planned Boolean effect")
        };
        assert_eq!(effect.text.as_deref(), Some(result));
        assert!(effect
            .expanded_gears
            .iter()
            .any(|gear| gear.kind_id == "units/converted-equals"));
        finish(session, *effect);
    }
}

#[test]
fn issue_demo_full_name_and_scoped_alias_render_exact_text() {
    for (imports, comparator) in [
        ("", "units/converted-equals"),
        ("with units/converted-equals as =?\n", "=?"),
    ] {
        let source = format!(
            r#"{imports}plot convert-pitch-demo {{
 operation: units/convert(source = 1kHz, to = Hz)
 exact: {comparator}(expected = 1000Hz)
 show: presentation/text
 operation.receipt >> exact.receipt
 exact.result >> (. ? "Exactly 1000 Hz" : "Conversion did not yield exactly 1000 Hz") >> show.text
}}."#
        );
        let (session, effect) =
            TourSession::prepare("browser/issue-demo", "boot/issue-demo", &source, 1).unwrap();
        let TourHostEffect::Manifestation(effect) = effect else {
            panic!("planned Text effect")
        };
        assert_eq!(effect.text.as_deref(), Some("Exactly 1000 Hz"));
        finish(session, *effect);
    }
}

#[test]
fn unit_value_crosses_literal_record_relay_and_projection_in_browser_kernel() {
    let source = r#"type Envelope = {
 value: Unit
}
plot unit-source (
 >> trigger: Scalar
 output: Unit >>
) = (kHz)
plot pack (
 >> input: Unit
 output: Envelope >>
) = ({ value: . })
plot relay (
 >> input: Envelope
 output: Envelope >>
) {
 input >> (.) >> output
}
plot unpack (
 >> input: Envelope
 output: Unit >>
) = (.value)
plot unit-transport-demo {
 trigger: scalar/literal(value = 1)
 source: unit-source
 packed: pack
 relayed: relay
 unpacked: unpack
 show: presentation/text
 trigger.value >> source.trigger
 source.output >> packed.input
 packed.output >> relayed.input
 relayed.output >> unpacked.input
 unpacked.output >> (. == kHz ? "Unit preserved" : "Unit changed") >> show.text
}."#;
    let (session, effect) =
        TourSession::prepare("browser/unit-transport", "boot/unit-transport", source, 1).unwrap();
    let TourHostEffect::Manifestation(effect) = effect else {
        panic!("planned Text effect")
    };
    assert_eq!(effect.text.as_deref(), Some("Unit preserved"));
    finish(session, *effect);
}

#[test]
fn installed_browser_comparator_rejects_forged_runtime_receipts() {
    const PORTS: usize = crate::installed_browser::BROWSER_PORTS_PER_GEAR;
    use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
    use conduit_kernel::PortId;
    let source = r#"plot comparator-forgery {
 operation: units/convert(source = 1kHz, to = Hz)
 exact: units/converted-equals(expected = 1000Hz)
 show: presentation/bool-value
 operation.receipt >> exact.receipt
 exact.result >> show.value
}."#;
    let (session, effect) = run(source);
    let producer = session.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == "units/convert")
        .unwrap();
    let comparator = session.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == "units/converted-equals")
        .unwrap();
    let bytes = conduit_plot::quantity_conversion::prepare_configuration(&producer.configuration)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    for index in [0, bytes.len() - 1] {
        let mut forged = bytes.clone();
        forged[index] ^= 1;
        let mut values = HostedValueStore::new(4, 8192, 16384).unwrap();
        let installation =
            crate::installed_browser::factory(&comparator.implementation_id).unwrap();
        let mut back = (installation.prepare)(comparator, &mut values).unwrap();
        let input = values.store(&forged).unwrap();
        let mut inputs = [None; PORTS];
        inputs[0] = Some(input);
        let mut canonical = [None; PORTS];
        canonical[0] = Some(forged.as_slice());
        let mut io = StepIo::test_frame(inputs, [false; PORTS], [Some(1); PORTS], None, 8);
        assert!(matches!(
            back.step(&mut io, &StepInputBytes::test_frame(canonical, None)),
            StepOutcome::Fail(_)
        ));
        assert!(io.test_output(PortId(0)).is_none());
    }
    finish(session, effect);
}

#[test]
fn authored_unit_snapshots_preserve_active_browser_play_meaning() {
    let snapshot = |scale: &str, coefficient: i128, exponent: i16| {
        format!(
            "unit smoot : Distance = {{ reference: m, scale: {scale} }}\n{}",
            source(
                "units/convert",
                "1smoot",
                "m",
                &converted(coefficient, exponent)
            )
        )
    };
    let old_source = snapshot("1.7018", 17018, -4);
    let new_source = snapshot("2", 2, 0);
    let (old_session, old_effect) = run(&old_source);
    let (new_session, new_effect) = run(&new_source);
    let admitted_quantity = |session: &TourSession| {
        let placement = session.fragments[0]
            .placements
            .iter()
            .find(|gear| gear.kind_id.as_str() == "units/convert")
            .unwrap();
        let ConfigurationValue::Quantity(value) = &placement.configuration[0].value else {
            panic!("typed authored Quantity configuration")
        };
        value.value()
    };
    let old_quantity = admitted_quantity(&old_session);
    let new_quantity = admitted_quantity(&new_session);
    assert_ne!(old_quantity.unit(), new_quantity.unit());
    let retained_in_new_unit = old_quantity.convert(new_quantity.unit()).unwrap();
    assert_eq!(retained_in_new_unit.coefficient(), 8509);
    assert_eq!(retained_in_new_unit.exponent(), -4);
    // Both independent admitted snapshots have executed their installed kernel.
    // Completing the old active Play after the new admission retains its result.
    finish(new_session, new_effect);
    finish(old_session, old_effect);
    let (old_session, old_effect) = run(&old_source);
    finish(old_session, old_effect);
}

#[test]
fn authored_unit_prefix_policies_are_checked_before_browser_play() {
    for (policy, quantity) in [
        ("none", "1ksmoot"),
        ("si", "1Mismoot"),
        ("binary", "1ksmoot"),
    ] {
        let source = format!(
            "unit smoot : Distance = {{ reference: m, scale: 1.7018, prefixes: {policy} }}\n{}",
            source("units/convert", quantity, "m", "true")
        );
        assert!(
            TourSession::prepare("browser/prefix-refusal", "boot/prefix-refusal", &source, 1)
                .is_err()
        );
    }
    let source = format!(
        "unit smoot : Distance = {{ reference: m, scale: 1.7018, prefixes: si }}\n{}",
        source("units/convert", "1ksmoot", "m", &converted(17018, -1))
    );
    let (session, effect) = run(&source);
    finish(session, effect);
}

#[test]
fn authored_custom_family_forwards_startup_and_crosses_record_runtime_ports() {
    let declarations = "dimension wobble\ntype Wobble = quantity { dimension: wobble }\nunit wob/s : Wobble = { reference: origin, scale: 1 }\nunit doublewob/s : Wobble = { reference: wob/s, scale: 2 }\n";
    let transport = format!(
        r#"{declarations}
 type Envelope = {{
  value: Wobble
 }}
 plot literal (
  seed: Wobble
  >> trigger: Scalar
  output: Wobble >>
 ) = (seed)
 plot pack (
  >> input: Wobble
  output: Envelope >>
 ) = ({{ value: . }})
 plot relay (
  >> input: Envelope
  output: Envelope >>
 ) {{
  input >> (.) >> output
 }}
 plot unpack (
  >> input: Envelope
  output: Wobble >>
 ) = (.value)
 plot custom-family-transport {{
  trigger: scalar/literal(value = 1)
  literal: literal(seed = 3doublewob/s)
  packed: pack
  relayed: relay
  unpacked: unpack
  show: presentation/bool-value
  trigger.value >> literal.trigger
  literal.output >> packed.input
  packed.output >> relayed.input
  relayed.output >> unpacked.input
  unpacked.output >> (. == 3doublewob/s) >> show.value
 }}."#
    );
    let (session, effect) =
        TourSession::prepare("browser/custom-family", "boot/custom-family", &transport, 1).unwrap();
    let TourHostEffect::Manifestation(effect) = effect else {
        panic!("custom family manifestation")
    };
    assert_eq!(effect.text.as_deref(), Some("true"));
    finish(session, *effect);
    let conversion = format!(
        r#"{declarations}
 plot forward (
  seed: Wobble
  target: Unit
  receipt: ExactQuantityConversionReceipt <= 8192B >>
 ) {{
  operation: units/convert(source = seed, to = target)
  operation.receipt >> receipt
 }}
 plot custom-family-conversion {{
  operation: forward(seed = 3doublewob/s, target = wob/s)
  exact: units/converted-equals(expected = 6wob/s)
  show: presentation/bool-value
  operation.receipt >> exact.receipt
  exact.result >> show.value
 }}."#
    );
    let (session, effect) = run(&conversion);
    finish(session, effect);
}
