//! Closed scalar configuration and law variants; full semantic validation follows.
use super::{json::*, Layout};
use conduit_core::*;
pub(super) fn contract(raw: &RawValue, l: &mut Layout) -> R<()> {
    let o = object(raw, &["configuration", "laws"], 3)?;
    array::<KindConfigurationField>(o.get(0)?, l, configuration)?;
    array::<KindSemanticLaw>(o.get(1)?, l, law)
}
fn configuration(raw: &RawValue, l: &mut Layout) -> R<()> {
    Layout::add_fact(&mut l.validation.configuration_keys, 1)?;
    let o = object(raw, &["key", "default_value", "rule"], 7)?;
    text(o.get(0)?, l)?;
    let (tag, value) = variant(o.get(1)?, &["Text", "U64"])?;
    l.validation.configuration_kind_bytes =
        l.validation.configuration_kind_bytes.max(if tag == 0 {
            TEXT_INFO_ID.len()
        } else {
            COUNT_INFO_ID.len()
        });
    match tag {
        0 => text(value, l)?,
        1 => unsigned(value, u64::MAX, true)?,
        _ => unreachable!(),
    }
    let (tag, rule) = variant(o.get(2)?, &["TextBytes", "DataLabel", "U64Range"])?;
    if tag < 2 {
        let rule = object(rule, &["maximum"], 1)?;
        unsigned(rule.get(0)?, u32::MAX.into(), false)
    } else {
        let rule = object(rule, &["minimum", "maximum"], 3)?;
        unsigned(rule.get(0)?, u64::MAX, true)?;
        unsigned(rule.get(1)?, u64::MAX, true)
    }
}
fn law(raw: &RawValue, l: &mut Layout) -> R<()> {
    let (tag, value) = variant(
        raw,
        &[
            "ValueContracts",
            "Terminal",
            "ExternalEffects",
            "TemporalState",
            "TimeDependence",
            "RandomDependence",
            "ResourceDependence",
            "Suspension",
            "Variability",
            "Replay",
            "TerminalTransduction",
        ],
    )?;
    match tag {
        0 => {
            l.current_value_locations = 0;
            l.current_value_location_bytes = 0;
            array::<FrontValueContract>(value, l, front_value)?;
            l.validation.maximum_value_locations = l
                .validation
                .maximum_value_locations
                .max(l.current_value_locations);
            l.validation.maximum_value_location_string_bytes = l
                .validation
                .maximum_value_location_string_bytes
                .max(l.current_value_location_bytes);
            Ok(())
        }
        1 => terminal(value),
        2 => word(value, &["None", "Observable"]).map(|_| ()),
        3 => word(
            value,
            &[
                "None",
                "Retained",
                "SourceSeededFinite",
                "SourceSeededFlowFinite",
            ],
        )
        .map(|_| ()),
        4..=6 => word(value, &["None", "ExplicitInput", "Ambient"]).map(|_| ()),
        7 => word(value, &["Never", "MaySuspend"]).map(|_| ()),
        8 => word(value, &["DeterministicFromInputs", "AdmittedVariability"]).map(|_| ()),
        9 => replay(value, l),
        10 => transduction(value, l),
        _ => unreachable!(),
    }
}
fn terminal(raw: &RawValue) -> R<()> {
    if raw.get().starts_with('"') {
        word(
            raw,
            &[
                "EmitsOnce",
                "EmitsOnceWhenScopeIsEligible",
                "CompletesAfterConfiguredCount",
                "CompletesWhenInputsClose",
                "MirrorsInputTerminal",
                "RetainsLatestUntilReleased",
                "EmitsThroughSourceTerminalFlag",
                "EmitsCurrentAndCompletesWhenInputCloses",
                "CoupledAtomicFanoutAndMirrorsInputTerminal",
                "FirstReadyLeftTieCancelsLoserOrCompletesWithoutWinner",
                "CurrentBooleanGateDefaultsClosedAndCompletesWhenInputsClose",
                "CurrentScalarSelectorCompletesWhenInputsClose",
                "EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible",
                "TrailingDebounceFlushesPendingValueThenCompletesWhenInputCloses",
                "InactivityStateCancelsDeadlineAndCompletesWhenInputCloses",
                "DelaysEachValueInOrderAndDrainsOnInputClosure",
                "LeadingThrottleDropsValuesDuringIntervalAndCompletesWhenInputCloses",
                "SamplesLatestValueAtCadenceAndCompletesWhenCadenceCloses",
                "SimulatedCurrentObservationEmitsOnce",
                "HostInputEndsOrFailsSource",
                "HostObservationEndsOrFailsSource",
                "EmitsInitialAndTogglesUntilInputCloses",
                "EmitsOneField",
                "EvolvesAfterTicksAndCompletesWhenTickCloses",
                "PresentsEachFieldAndCompletesWhenInputCloses",
                "CompletesAfterDockedRefusedOrDeadline",
            ],
        )?;
        Ok(())
    } else {
        let (tag, v) = variant(
            raw,
            &["CompletesAfterFixedCount", "TumblingProcessingTimeWindow"],
        )?;
        let o = object(v, &[if tag == 0 { "count" } else { "maximum_items" }], 1)?;
        unsigned(
            o.get(0)?,
            if tag == 0 { u64::MAX } else { u16::MAX.into() },
            false,
        )
    }
}
fn replay(raw: &RawValue, l: &mut Layout) -> R<()> {
    if raw.get().starts_with('"') {
        word(raw, &["Exact", "Ineligible"])?;
        Ok(())
    } else {
        let (tag, v) = variant(raw, &["Idempotent", "Transactional", "Compensatable"])?;
        let o = object(
            v,
            &[match tag {
                0 => "operation_key_kind",
                1 => "transaction_contract",
                _ => "compensation_contract",
            }],
            1,
        )?;
        text(o.get(0)?, l)
    }
}
fn front_value(raw: &RawValue, l: &mut Layout) -> R<()> {
    let o = object(raw, &["location", "contract"], 3)?;
    let (_, v) = variant(
        o.get(0)?,
        &[
            "Startup",
            "Input",
            "Output",
            "InputAbnormal",
            "OutputAbnormal",
        ],
    )?;
    let bytes = text_bytes(v, l)?;
    Layout::add_fact(&mut l.current_value_locations, 1)?;
    Layout::add_fact(&mut l.current_value_location_bytes, bytes)?;
    let c = object(
        o.get(1)?,
        &["value_kind", "maximum_bytes", "constraints"],
        7,
    )?;
    text(c.get(0)?, l)?;
    unsigned(c.get(1)?, u32::MAX.into(), false)?;
    array::<ValueConstraint>(c.get(2)?, l, constraint)
}
fn constraint(raw: &RawValue, _l: &mut Layout) -> R<()> {
    let (_, v) = variant(raw, &["UnsignedRange"])?;
    let o = object(
        v,
        &["minimum", "maximum", "minimum_endpoint", "maximum_endpoint"],
        15,
    )?;
    for i in [0, 1] {
        if !null(o.get(i)?) {
            unsigned(o.get(i)?, u64::MAX, false)?;
        }
    }
    for i in [2, 3] {
        word(o.get(i)?, &["Inclusive", "Exclusive"])?;
    }
    Ok(())
}
fn transduction(raw: &RawValue, l: &mut Layout) -> R<()> {
    Layout::add_fact(&mut l.validation.terminal_inputs, 1)?;
    let o = object(
        raw,
        &[
            "input_port_id",
            "output_port_id",
            "normal_close",
            "abnormal",
            "cancellation",
        ],
        31,
    )?;
    text(o.get(0)?, l)?;
    text(o.get(1)?, l)?;
    for (i, units, payloads) in [
        (
            2,
            &[
                "NotAccepted",
                "PropagateAfterDrain",
                "Consume",
                "PropagateWhenAllClose",
            ][..],
            &[
                "FlushThenPropagate",
                "FlushThenPropagateWhenAllClose",
                "DomainSpecific",
            ][..],
        ),
        (
            3,
            &["NotAccepted", "PropagateAfterDrain", "Recover"][..],
            &["FinalizeThenPropagate", "DomainSpecific"][..],
        ),
    ] {
        let raw = o.get(i)?;
        if raw.get().starts_with('"') {
            word(raw, units)?;
        } else {
            let (tag, value) = variant(raw, payloads)?;
            if payloads[tag] == "DomainSpecific" {
                let field = object(value, &["law"], 1)?;
                text(field.get(0)?, l)?;
            } else {
                let fields = object(value, &["maximum_items", "maximum_bytes"], 3)?;
                unsigned(fields.get(0)?, u16::MAX.into(), false)?;
                unsigned(fields.get(1)?, u32::MAX.into(), false)?;
            }
        }
    }
    let raw = o.get(4)?;
    if raw.get().starts_with('"') {
        word(raw, &["NotCancellable"])?;
    } else {
        let (tag, value) = variant(raw, &["Request", "DomainSpecific"])?;
        let field = object(
            value,
            &[if tag == 0 { "disposition_kind" } else { "law" }],
            1,
        )?;
        text(field.get(0)?, l)?;
    }
    Ok(())
}
