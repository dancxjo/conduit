use super::*;
use conduit_core::ExpandedPlotId;

impl ExpandedCanonicalPlot {
    pub fn validate_expansion(&self) -> Result<(), CanonicalExpansionDiagnostic> {
        let plot = CheckedCanonicalPlot {
            checked_plot_id: self.checked_plot_id.clone(),
            name: self.name.clone(),
            completion: self.completion,
            startup_parameters: vec![],
            runtime_ports: Vec::new(),
            action_bindings: Vec::new(),
            face_fragment_output: None,
            runtime_front: conduit_core::CheckedFront::new(vec![], vec![], vec![], None),
            shorthand: None,
            local_values: Vec::new(),
            pools: Vec::new(),
            gears: Vec::new(),
            cords: Vec::new(),
        };
        let expected = expanded_identity(
            &plot,
            &self.gears,
            &self.connections,
            &self.shared_pools,
            &self.provenance,
            &self.realization_backs,
            &self.activations,
        );
        if self.expanded_plot_id != expected {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-049",
                "expanded plot identity differs from its canonical primitive graph".into(),
            ));
        }
        if self.provenance_digest != provenance_digest(&self.source_document_id, &self.provenance) {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-049",
                "expanded source provenance differs from its exact source document mapping".into(),
            ));
        }
        if self
            .realization_backs
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-049",
                "expanded Back selections must have unique canonical ordering".into(),
            ));
        }
        let gears = self
            .gears
            .iter()
            .map(|gear| (gear.gear_id.clone(), gear))
            .collect::<BTreeMap<_, _>>();
        if gears.len() != self.gears.len() {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-038",
                "expanded gear paths are not unique".into(),
            ));
        }
        let mut activation_ids = BTreeSet::new();
        for activation in &self.activations {
            let owner = gears.get(&activation.owner_gear_id);
            let owner_matches = owner.is_some_and(|owner| {
                let mut input = activation.input.clone();
                input.temporal = conduit_core::PortTemporal::Flow { closes: true };
                let mut output = match activation.mode {
                    crate::ActivationSyntax::Each { .. } => activation.output.clone(),
                    crate::ActivationSyntax::Select { .. } => {
                        let Some(output) = owner.outputs.first() else {
                            return false;
                        };
                        let mut retained = activation.input.clone();
                        retained.port_id = output.port_id.clone();
                        retained.direction = conduit_core::PortDirection::Output;
                        retained
                    }
                    crate::ActivationSyntax::Fold { .. } | crate::ActivationSyntax::Scan { .. } => {
                        activation.output.clone()
                    }
                };
                output.temporal = if matches!(activation.mode, crate::ActivationSyntax::Fold { .. })
                {
                    conduit_core::PortTemporal::Value
                } else {
                    conduit_core::PortTemporal::Flow { closes: true }
                };
                owner.inputs == [input] && owner.outputs == [output]
            });
            let fold_metadata_matches = match activation.mode {
                crate::ActivationSyntax::Fold { .. } | crate::ActivationSyntax::Scan { .. } => {
                    activation
                        .accumulator_input
                        .as_ref()
                        .is_some_and(|accumulator| {
                            accumulator.port_id.as_str() == "accumulator"
                                && accumulator.temporal == conduit_core::PortTemporal::Value
                                && accumulator.direction == conduit_core::PortDirection::Input
                                && accumulator.value_kind == activation.output.value_kind
                                && accumulator.abnormal_kind == activation.output.abnormal_kind
                        })
                        && activation
                            .initial_accumulator
                            .as_ref()
                            .is_some_and(|initial| {
                                !matches!(
                                    initial,
                                    crate::CanonicalStartupValue::PlotParameter(_)
                                        | crate::CanonicalStartupValue::PoolReference(_)
                                ) && !matches!(
                                    initial,
                                    crate::CanonicalStartupValue::Structured(value)
                                        if value.try_concrete().is_none()
                                )
                            })
                        && activation.initial_accumulator_bytes.is_some()
                }
                _ => {
                    activation.accumulator_input.is_none()
                        && activation.initial_accumulator.is_none()
                        && activation.initial_accumulator_bytes.is_none()
                }
            };
            if !activation_ids.insert(activation.activation_id.as_str())
                || !owner_matches
                || !fold_metadata_matches
                || activation.input.temporal != conduit_core::PortTemporal::Value
                || activation.output.temporal != conduit_core::PortTemporal::Value
                || activation.input.direction != conduit_core::PortDirection::Input
                || activation.output.direction != conduit_core::PortDirection::Output
                || activation.input.abnormal_kind != activation.output.abnormal_kind
                || activation.selected_plot.is_empty()
                || activation.selected_checked_plot_id.as_str().is_empty()
            {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-049",
                    "expanded activation differs from its exact selected Value Plot".into(),
                ));
            }
        }
        for connection in &self.connections {
            let source = gears.get(&connection.source_gear_id).and_then(|gear| {
                gear.outputs
                    .iter()
                    .find(|port| port.port_id == connection.source_port_id)
            });
            let sink = gears.get(&connection.sink_gear_id).and_then(|gear| {
                gear.inputs
                    .iter()
                    .find(|port| port.port_id == connection.sink_port_id)
            });
            let source_matches = source.is_some_and(|port| match connection.track {
                conduit_core::ConnectionTrack::Payload => {
                    port.value_kind == connection.value_kind && port.temporal == connection.temporal
                }
                conduit_core::ConnectionTrack::NormalClose => {
                    matches!(
                        port.temporal,
                        conduit_core::PortTemporal::Flow { closes: true }
                    ) && connection.value_kind.as_str() == conduit_core::UNIT_INFO_ID
                        && connection.temporal == conduit_core::PortTemporal::Value
                }
                conduit_core::ConnectionTrack::AbnormalTerminal => {
                    port.abnormal_kind.as_ref() == Some(&connection.value_kind)
                        && connection.temporal == conduit_core::PortTemporal::Value
                }
                conduit_core::ConnectionTrack::Quiescence => {
                    matches!(port.temporal, conduit_core::PortTemporal::Flow { .. })
                        && connection.value_kind.as_str() == conduit_core::UNIT_INFO_ID
                        && connection.temporal == conduit_core::PortTemporal::Value
                }
            });
            let sink_matches = sink.is_some_and(|port| {
                port.value_kind == connection.value_kind
                    && (port.temporal == connection.temporal
                        || matches!(
                            (connection.temporal, port.temporal),
                            (
                                conduit_core::PortTemporal::Flow { .. },
                                conduit_core::PortTemporal::Value
                            )
                        )
                        || matches!(
                            (connection.temporal, port.temporal),
                            (
                                conduit_core::PortTemporal::Flow { closes: true },
                                conduit_core::PortTemporal::Flow { closes: false }
                            )
                        ))
            });
            if !source_matches || !sink_matches {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-049",
                    "expanded cord differs from its exact primitive port contracts".into(),
                ));
            }
        }
        for (index, pool) in self.shared_pools.iter().enumerate() {
            if self.shared_pools[..index]
                .iter()
                .any(|prior| prior.pool_id == pool.pool_id)
            {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-049",
                    "expanded shared-pool identities are not unique".into(),
                ));
            }
            let mut consumers = self
                .gears
                .iter()
                .filter(|gear| gear.pool_references.contains(&pool.pool_id))
                .map(|gear| gear.gear_id.clone())
                .collect::<Vec<_>>();
            consumers.sort();
            if consumers.is_empty() || consumers != pool.consumers {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-049",
                    "expanded shared-pool consumers differ from explicit bindings".into(),
                ));
            }
        }
        let provenance_ids = self
            .provenance
            .iter()
            .map(|row| row.gear_id.as_str())
            .collect::<BTreeSet<_>>();
        if provenance_ids.len() != self.provenance.len()
            || provenance_ids.len() != self.gears.len()
            || !self
                .gears
                .iter()
                .all(|gear| provenance_ids.contains(gear.gear_id.as_str()))
        {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-049",
                "expanded provenance must name every primitive gear exactly once".into(),
            ));
        }
        Ok(())
    }
}

pub(super) fn expanded_identity(
    plot: &CheckedCanonicalPlot,
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
    shared_pools: &[ExpandedSharedPool],
    provenance: &[ExpandedGearProvenance],
    realization_backs: &[conduit_core::PlotBack],
    activations: &[ExpandedActivation],
) -> ExpandedPlotId {
    let mut canonical = format!("canonical-expanded:{}", plot.checked_plot_id.as_str());
    for gear in gears {
        push(&mut canonical, gear.gear_id.as_str());
        push(&mut canonical, gear.kind_id.as_str());
        push(&mut canonical, gear.kind_contract_revision.as_str());
        for parameter in &gear.startup_parameters {
            push(&mut canonical, &parameter.name);
            push(&mut canonical, parameter.value_type.as_str());
            push(
                &mut canonical,
                if parameter.has_default {
                    "default"
                } else {
                    "required"
                },
            );
        }
        if let Some((input, output)) = &gear.shorthand {
            push(&mut canonical, input.as_str());
            push(&mut canonical, output.as_str());
        }
        for port in gear.inputs.iter().chain(&gear.outputs) {
            push(&mut canonical, port.port_id.as_str());
            push(&mut canonical, port.value_kind.as_str());
            push(&mut canonical, port.temporal.as_str());
            push(
                &mut canonical,
                port.abnormal_kind
                    .as_ref()
                    .map_or("none", conduit_core::KindId::as_str),
            );
            push(
                &mut canonical,
                match port.direction {
                    conduit_core::PortDirection::Input => "input",
                    conduit_core::PortDirection::Output => "output",
                },
            );
        }
        crate::push_terminal_transduction_text(&mut canonical, &gear.terminal_transductions);
        for resource in &gear.resource_ports {
            push(&mut canonical, resource.port_id.as_str());
            push(&mut canonical, resource.class_id.as_str());
            push(&mut canonical, &format!("{:?}", resource.ownership));
            push(&mut canonical, &format!("{:?}", resource.lifecycle));
            push(&mut canonical, &format!("{:?}", resource.mobility));
        }
        for entry in &gear.configuration {
            push(&mut canonical, &entry.key);
            match entry.value {
                conduit_core::ConfigurationValue::Bool(value) => {
                    push(&mut canonical, "bool");
                    push(&mut canonical, if value { "true" } else { "false" });
                }
                conduit_core::ConfigurationValue::U64(value) => {
                    push(&mut canonical, "u64");
                    push(&mut canonical, &value.to_string());
                }
                conduit_core::ConfigurationValue::I64(value) => {
                    push(&mut canonical, "i64-scalar-microunits");
                    push(&mut canonical, &value.to_string());
                }
                conduit_core::ConfigurationValue::Text(ref value) => {
                    push(&mut canonical, "text");
                    push(&mut canonical, value);
                }
                conduit_core::ConfigurationValue::Structured(ref value) => {
                    push(&mut canonical, "structured");
                    push(&mut canonical, value.profile().as_str());
                    push(&mut canonical, &hex(value.canonical_value()));
                }
                conduit_core::ConfigurationValue::Quantity(value) => {
                    push(&mut canonical, "quantity");
                    push(&mut canonical, &hex(&value.encode()));
                }
            }
        }
        for pool in &gear.pool_references {
            push(&mut canonical, "pool-reference");
            push(&mut canonical, pool.as_str());
        }
    }
    for connection in connections {
        push(&mut canonical, connection.source_gear_id.as_str());
        push(&mut canonical, connection.source_port_id.as_str());
        push(&mut canonical, connection.sink_gear_id.as_str());
        push(&mut canonical, connection.sink_port_id.as_str());
        push(&mut canonical, connection.value_kind.as_str());
        push(&mut canonical, connection.track.as_str());
        push(&mut canonical, connection.temporal.as_str());
    }
    for pool in shared_pools {
        push(&mut canonical, pool.pool_id.as_str());
        push(&mut canonical, pool.declaration_id.as_str());
        push(&mut canonical, &pool.maximum_members.to_string());
        for parameter in pool.member_front.startup_parameters() {
            push(&mut canonical, &parameter.name);
            push(&mut canonical, parameter.value_type.as_str());
            push(
                &mut canonical,
                if parameter.has_default {
                    "default"
                } else {
                    "required"
                },
            );
        }
        for port in pool
            .member_front
            .inputs()
            .iter()
            .chain(pool.member_front.outputs())
        {
            push(&mut canonical, port.port_id.as_str());
            push(&mut canonical, port.value_kind.as_str());
            push(&mut canonical, port.temporal.as_str());
            push(
                &mut canonical,
                port.abnormal_kind
                    .as_ref()
                    .map_or("none", conduit_core::KindId::as_str),
            );
            push(&mut canonical, &format!("{:?}", port.direction));
        }
        for consumer in &pool.consumers {
            push(&mut canonical, consumer.as_str());
        }
    }
    for row in provenance {
        push(&mut canonical, &row.gear_id);
        push(&mut canonical, &row.plot_path.join("/"));
        push(&mut canonical, &row.source_plot);
        push(&mut canonical, &row.source_gear);
    }
    for back in realization_backs {
        push(&mut canonical, "realization-back");
        push(&mut canonical, &back.invocation_path);
        push(&mut canonical, back.kind_id.as_str());
        push(&mut canonical, back.kind_contract_revision.as_str());
        push(&mut canonical, back.source_document_id.as_str());
        push(&mut canonical, back.checked_plot_id.as_str());
    }
    for activation in activations {
        push(
            &mut canonical,
            match activation.mode {
                crate::ActivationSyntax::Each { .. } => "activation-each",
                crate::ActivationSyntax::Select { .. } => "activation-select",
                crate::ActivationSyntax::Fold { .. } => "activation-fold",
                crate::ActivationSyntax::Scan { .. } => "activation-scan",
            },
        );
        push(&mut canonical, &activation.mode.maximum_items().to_string());
        push(&mut canonical, &activation.activation_id);
        push(&mut canonical, activation.owner_gear_id.as_str());
        push(&mut canonical, &activation.selected_plot);
        push(&mut canonical, activation.selected_checked_plot_id.as_str());
        for port in [&activation.input, &activation.output] {
            push(&mut canonical, port.port_id.as_str());
            push(&mut canonical, port.value_kind.as_str());
            push(&mut canonical, port.temporal.as_str());
            push(
                &mut canonical,
                port.abnormal_kind
                    .as_ref()
                    .map_or("none", conduit_core::KindId::as_str),
            );
        }
        if let Some(accumulator) = &activation.accumulator_input {
            push(&mut canonical, accumulator.port_id.as_str());
            push(&mut canonical, accumulator.value_kind.as_str());
        }
        if let Some(initial) = &activation.initial_accumulator {
            push(
                &mut canonical,
                &crate::syntax_identity::canonical_value(initial),
            );
        }
        if let Some(bytes) = &activation.initial_accumulator_bytes {
            push(&mut canonical, &hex(bytes));
        }
    }
    ExpandedPlotId::from(hash_string(&canonical))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub(super) fn provenance_digest(
    source_document_id: &conduit_core::SourceDocumentId,
    provenance: &[ExpandedGearProvenance],
) -> String {
    let mut canonical = String::from("canonical-expansion-provenance");
    push(&mut canonical, source_document_id.as_str());
    for row in provenance {
        push(&mut canonical, &row.gear_id);
        push(&mut canonical, &row.plot_path.join("/"));
        push(&mut canonical, &row.source_plot);
        push(&mut canonical, &row.source_gear);
        push(&mut canonical, &row.source_span.start.to_string());
        push(&mut canonical, &row.source_span.end.to_string());
        push(&mut canonical, &row.source_span.line.to_string());
        push(&mut canonical, &row.source_span.column.to_string());
    }
    hash_string(&canonical)
}

fn push(target: &mut String, value: &str) {
    target.push_str(&value.len().to_string());
    target.push(':');
    target.push_str(value);
}
