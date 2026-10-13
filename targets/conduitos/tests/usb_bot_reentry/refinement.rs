//! Minimal exact member-forwarding probes. These compile the candidate's actual
//! checked Types/specializations, with no altered byte laws or native providers.
use super::*;

#[test]
fn refined_reentry_members_forward_through_exact_typed_pair() {
    let source = source_with(
        "type ReentryBankCarrier = {\n bank: Bytes <= 2048B\n}\ntype ReentryDataCarrier = {\n data: UsbEndpointReadResult\n}\ntype ReentryStateCarrier = {\n previous: BotReentryState\n}\ntype ReentryEventCarrier = {\n event: BotReentryEvent\n}\nplot probe-bank (\n pair: BotReentryPair...| >> carrier: ReentryBankCarrier...|\n) = ({bank: .0.bank})\nplot probe-data (\n pair: BotReentryPair...| >> carrier: ReentryDataCarrier...|\n) = ({data: .0.data})\nplot probe-state (\n pair: BotReentryPair...| >> carrier: ReentryStateCarrier...|\n) = ({previous: .0})\nplot probe-event (\n pair: BotReentryPair...| >> carrier: ReentryEventCarrier...|\n) = ({event: .1})",
    );
    let mut refused = Vec::new();
    for name in ["probe-bank", "probe-data", "probe-state", "probe-event"] {
        if let Err(error) = source.expand(name) {
            refused.push(format!("{name}: {error:?}"));
        }
    }
    assert!(
        refused.is_empty(),
        "exact refined member failures: {}",
        refused.join("; ")
    );
}

#[test]
fn refined_variant_tag_observation_does_not_construct_a_native_value() {
    let source = source_with(
        "plot probe-fixed-transition (\n pair: BotReentryPair...| >> carrier: BotReentryTransition...|\n) = ({previous: .0, event: .1, accepted: false})\nplot probe-event-tag (\n pair: BotReentryPair...| >> observed: Boolean...|\n) = (.1 is read)\nplot probe-read-tag (\n pair: BotReentryPair...| >> observed: Boolean...|\n) = (.0.data is completed)\nplot probe-phase-predicate (\n pair: BotReentryPair...| >> observed: Boolean...|\n) = (.0.phase == 5)",
    );
    let mut refused = Vec::new();
    for name in [
        "probe-fixed-transition",
        "probe-event-tag",
        "probe-read-tag",
        "probe-phase-predicate",
    ] {
        if let Err(error) = source.expand(name) {
            refused.push(format!("{name}: {error:?}"));
        }
    }
    assert!(
        refused.is_empty(),
        "exact predicate/construction failures: {}",
        refused.join("; ")
    );
}

fn strip_outer(mut value: &str) -> &str {
    loop {
        value = value.trim();
        if !value.starts_with('(') {
            return value;
        }
        let mut depth = 0;
        let mut last = 0;
        for (index, byte) in value.bytes().enumerate() {
            match byte {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        last = index;
                        break;
                    }
                }
                _ => {}
            }
        }
        if last != value.len() - 1 {
            return value;
        }
        value = &value[1..last];
    }
}
fn split_top(value: &str, needle: &str) -> Option<usize> {
    let mut depth = 0;
    for (index, byte) in value.bytes().enumerate() {
        match byte {
            b'(' | b'{' | b'[' => depth += 1,
            b')' | b'}' | b']' => depth -= 1,
            _ => {}
        }
        if depth == 0 && value[index..].starts_with(needle) {
            return Some(index);
        }
    }
    None
}
fn branches<'a>(value: &'a str, output: &mut Vec<&'a str>) {
    let value = strip_outer(value);
    if let Some(index) = split_top(value, "||") {
        branches(&value[..index], output);
        branches(&value[index + 2..], output);
    } else {
        output.push(value);
    }
}
#[test]
fn exact_accept_predicate_and_each_phase_branch_expand_without_refined_construction() {
    let text = include_str!("../../plots/usb/mass-storage-read-reentry.conduit");
    let expression = text
        .split("plot bot-reentry-accept (")
        .nth(1)
        .unwrap()
        .split("plot bot-reentry-advance (")
        .next()
        .unwrap();
    let accepted = expression
        .split("accepted: ")
        .nth(1)
        .unwrap()
        .trim()
        .strip_suffix(')')
        .unwrap()
        .trim()
        .strip_suffix('}')
        .unwrap()
        .trim();
    let guard = strip_outer(accepted);
    let colon = split_top(guard, ":").unwrap();
    let mut phases = Vec::new();
    branches(&guard[colon + 1..], &mut phases);
    assert_eq!(
        phases.len(),
        10,
        "probe must cover all authored groups including post-authorizer permit gates"
    );
    let mut extra = format!(
        "plot probe-accepted-bool (\n pair: BotReentryPair...| >> observed: Boolean...|\n) = ({accepted})\n"
    );
    for (index, predicate) in phases.iter().enumerate() {
        extra.push_str(&format!("plot probe-branch-{index} (\n pair: BotReentryPair...| >> observed: Boolean...|\n) = ({predicate})\n"));
    }
    let source = source_with(&extra);
    let mut refused = Vec::new();
    for name in std::iter::once("probe-accepted-bool".into())
        .chain((0..phases.len()).map(|n| format!("probe-branch-{n}")))
    {
        if let Err(error) = source.expand(&name) {
            refused.push(format!("{name}: {error:?}"));
        }
    }
    assert!(
        refused.is_empty(),
        "exact accepted predicate failures: {}",
        refused.join("; ")
    );
}

fn expression_depth(node: &conduit_plot::PortableExpressionNode) -> usize {
    use conduit_plot::PortableExpressionOperation as Op;
    let child_depth = match &node.operation {
        Op::Projection { value, .. } | Op::Unary { operand: value, .. } => {
            Some(expression_depth(value))
        }
        Op::Binary { left, right, .. } => Some(expression_depth(left).max(expression_depth(right))),
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => Some(
            expression_depth(condition)
                .max(expression_depth(when_true))
                .max(expression_depth(when_false)),
        ),
        Op::Record(fields) => fields
            .iter()
            .map(|(_, value)| expression_depth(value))
            .max(),
        Op::Tuple(values)
        | Op::Collection(values)
        | Op::SemanticCall {
            arguments: values, ..
        } => values.iter().map(expression_depth).max(),
        Op::Variant { payload, .. } => Some(expression_depth(payload)),
        Op::Input | Op::Literal(_) | Op::CanonicalLiteral(_) | Op::Constant(_) => None,
    };
    child_depth.map_or(0, |depth| depth + 1)
}
#[test]
fn complete_accept_program_respects_actual_portable_decoder_depth_without_widening_it() {
    let source = source();
    let accepted = program(&source, "bot-reentry-accept");
    let depth = expression_depth(&accepted.root);
    assert!(
        depth <= conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH,
        "actual accept AST depth {depth} exceeds unchanged decoder bound"
    );
    let encoded = accepted.canonical_bytes().unwrap();
    assert_eq!(
        conduit_plot::PortableExpressionProgram::from_canonical_bytes(&encoded).unwrap(),
        accepted
    );
}

#[test]
fn actual_accept_scalar_and_record_wrapper_report_exact_decoder_boundary() {
    use conduit_plot::{PortableExpressionOperation as Op, PortableExpressionProgram};
    let text = include_str!("../../plots/usb/mass-storage-read-reentry.conduit");
    let expression = text
        .split("plot bot-reentry-accept (")
        .nth(1)
        .unwrap()
        .split("plot bot-reentry-advance (")
        .next()
        .unwrap();
    let accepted = expression
        .split("accepted: ")
        .nth(1)
        .unwrap()
        .trim()
        .strip_suffix(')')
        .unwrap()
        .trim()
        .strip_suffix('}')
        .unwrap()
        .trim();
    let extra = format!(
        "plot measured-accept-bool (\n pair: BotReentryPair...| >> observed: Boolean...|\n) = ({accepted})\nplot measured-fixed-transition (\n pair: BotReentryPair...| >> carrier: BotReentryTransition...|\n) = ({{previous: .0, event: .1, accepted: false}})\n"
    );
    let source = source_with(&extra);
    let scalar = program(&source, "measured-accept-bool");
    let mut wrapped = program(&source, "measured-fixed-transition");
    assert_eq!(scalar.input_type, wrapped.input_type);
    let Op::Record(fields) = &mut wrapped.root.operation else {
        panic!("record wrapper")
    };
    fields
        .iter_mut()
        .find(|(name, _)| name == "accepted")
        .unwrap()
        .1 = scalar.root.clone();
    let scalar_depth = expression_depth(&scalar.root);
    let record_depth = expression_depth(&wrapped.root);
    let encoded = wrapped.canonical_bytes().unwrap();
    let decoded = PortableExpressionProgram::from_canonical_bytes(&encoded);
    assert!(
        decoded.is_ok(),
        "actual scalar depth={scalar_depth}; actual record depth={record_depth}; bound={}; encoded bytes={}; exact decoder refusal={:?}",
        conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH,
        encoded.len(),
        decoded.err()
    );
}

#[test]
fn every_reentry_pure_plot_expands_with_its_actual_typed_carrier() {
    let source = source();
    let text = include_str!("../../plots/usb/mass-storage-read-reentry.conduit");
    let mut refused = Vec::new();
    for definition in text.split("plot ").skip(1) {
        let name = definition.split_whitespace().next().unwrap();
        if name == "bot-storage-read-reentry" || name == "bot-reentry-read-event" {
            continue;
        }
        if let Err(error) = source.expand(name) {
            refused.push(format!("{name}: {error:?}"));
        }
    }
    assert!(
        refused.is_empty(),
        "exact pure plot failures: {}",
        refused.join("; ")
    );
}
