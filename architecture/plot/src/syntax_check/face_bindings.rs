//! Source-time action and Face output bindings. These are checked meaning,
//! not an installed ingress, scheduler, or a claim that a Host can execute it.
use crate::checked_syntax::{CheckedActionBinding, StartupCatalog, SyntaxCheckDiagnostic};
use crate::syntax::{PlotFront, RuntimePortDirection, RuntimePortTemporal};
use alloc::{collections::BTreeSet, format, string::String, vec::Vec};
use conduit_core::{kind_id, port_id};

const MAX_ACTION_BINDINGS: usize = 32;
const MAX_ACTION_VALUE_BYTES: u64 = 4_096;
const MAX_FRAGMENT_BYTES: u64 = 65_536;

pub(super) fn check(
    front: &PlotFront,
    catalog: &StartupCatalog,
) -> Result<(Vec<CheckedActionBinding>, Option<String>), SyntaxCheckDiagnostic> {
    if front.action_bindings.len() > MAX_ACTION_BINDINGS {
        return Err(refuse(
            front.action_bindings[MAX_ACTION_BINDINGS].span,
            "too many Face action bindings",
        ));
    }
    let mut intents = BTreeSet::new();
    let mut inputs = BTreeSet::new();
    let mut checked = Vec::with_capacity(front.action_bindings.len());
    for binding in &front.action_bindings {
        if !intents.insert(binding.intent.text.as_str()) {
            return Err(refuse(binding.intent.span, "duplicate Face action intent"));
        }
        if !inputs.insert(binding.input.text.as_str()) {
            return Err(refuse(
                binding.input.span,
                "ambiguous Face action input port",
            ));
        }
        let port = front
            .runtime_ports
            .iter()
            .find(|port| port.name.text == binding.input.text)
            .ok_or_else(|| refuse(binding.input.span, "Face action input port is missing"))?;
        if port.direction != RuntimePortDirection::Input
            || !matches!(port.temporal, RuntimePortTemporal::Flow { closes: false })
        {
            return Err(refuse(
                binding.input.span,
                "Face action requires an open input Flow port",
            ));
        }
        let expected = crate::value_type::checked_value_kind(&binding.value_type.text, catalog)
            .map_err(|_| {
                refuse(
                    binding.value_type.span,
                    "Face action argument type is invalid",
                )
            })?;
        let actual = crate::value_type::checked_value_kind(&port.value_type.text, catalog)
            .map_err(|_| refuse(port.value_type.span, "Face action input type is invalid"))?;
        if expected != actual
            || (binding.argument.text == "target" && actual != kind_id("value/text"))
        {
            return Err(refuse(
                binding.value_type.span,
                "Face action argument and input types differ",
            ));
        }
        let Some(maximum) = port
            .maximum_bytes
            .filter(|value| *value > 0 && *value <= MAX_ACTION_VALUE_BYTES)
        else {
            return Err(refuse(
                port.span,
                "Face action input requires a finite byte bound",
            ));
        };
        checked.push(CheckedActionBinding {
            intent: binding.intent.text.clone(),
            argument: binding.argument.text.clone(),
            value_kind: actual,
            input_port: port_id(&binding.input.text),
            maximum_bytes: maximum as u32,
        });
    }
    let face_output = front
        .face_fragment_output
        .as_ref()
        .map(|output| {
            let port = front
                .runtime_ports
                .iter()
                .find(|port| port.name.text == output.text)
                .ok_or_else(|| refuse(output.span, "Face fragment output port is missing"))?;
            if port.direction != RuntimePortDirection::Output
                || !matches!(port.temporal, RuntimePortTemporal::Flow { closes: false })
            {
                return Err(refuse(
                    output.span,
                    "Face fragment requires an open output Flow port",
                ));
            }
            let actual = crate::value_type::checked_value_kind(&port.value_type.text, catalog)
                .map_err(|_| {
                    refuse(port.value_type.span, "Face fragment output type is invalid")
                })?;
            if actual != kind_id("face/fragment@1") {
                return Err(refuse(
                    port.value_type.span,
                    "Face fragment output has the wrong type",
                ));
            }
            if !port
                .maximum_bytes
                .is_some_and(|value| value > 0 && value <= MAX_FRAGMENT_BYTES)
            {
                return Err(refuse(
                    port.span,
                    "Face fragment output requires a finite byte bound",
                ));
            }
            Ok(output.text.clone())
        })
        .transpose()?;
    if !checked.is_empty() && face_output.is_none() {
        return Err(refuse(
            front.action_bindings[0].span,
            "Face actions require one fragment output",
        ));
    }
    Ok((checked, face_output))
}

fn refuse(span: crate::Span, message: &str) -> SyntaxCheckDiagnostic {
    SyntaxCheckDiagnostic {
        code: "CND-FRM-081",
        span,
        message: format!("Face binding: {message}"),
    }
}
