use super::*;
use conduit_core::ExpandedFormId;

impl ExpandedCanonicalForm {
    pub fn validate_expansion(&self) -> Result<(), CanonicalExpansionDiagnostic> {
        let form = CheckedCanonicalForm {
            checked_form_id: self.checked_form_id.clone(),
            name: self.name.clone(),
            startup_parameters: vec![],
            runtime_ports: Vec::new(),
            shorthand: None,
            local_values: Vec::new(),
            pools: Vec::new(),
            gears: Vec::new(),
            cords: Vec::new(),
        };
        let expected = expanded_identity(
            &form,
            &self.gears,
            &self.connections,
            &self.shared_pools,
            &self.provenance,
        );
        if self.expanded_form_id != expected {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-049",
                "expanded form identity differs from its canonical primitive graph".into(),
            ));
        }
        if self.provenance_digest != provenance_digest(&self.source_document_id, &self.provenance) {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-049",
                "expanded source provenance differs from its exact source document mapping".into(),
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
            if source.map(|port| (&port.value_kind, port.temporal))
                != Some((&connection.value_kind, connection.temporal))
                || sink.map(|port| (&port.value_kind, port.temporal))
                    != Some((&connection.value_kind, connection.temporal))
            {
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
    form: &CheckedCanonicalForm,
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
    shared_pools: &[ExpandedSharedPool],
    provenance: &[ExpandedGearProvenance],
) -> ExpandedFormId {
    let mut canonical = format!("canonical-expanded:{}", form.checked_form_id.as_str());
    for gear in gears {
        push(&mut canonical, gear.gear_id.as_str());
        push(&mut canonical, gear.kind_id.as_str());
        push(&mut canonical, gear.kind_contract_revision.as_str());
        for parameter in &gear.startup_parameters {
            push(&mut canonical, &parameter.name);
            push(&mut canonical, &parameter.value_type);
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
                match port.direction {
                    conduit_core::PortDirection::Input => "input",
                    conduit_core::PortDirection::Output => "output",
                },
            );
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
        push(&mut canonical, connection.temporal.as_str());
    }
    for pool in shared_pools {
        push(&mut canonical, pool.pool_id.as_str());
        push(&mut canonical, pool.declaration_id.as_str());
        push(&mut canonical, &pool.maximum_members.to_string());
        for parameter in pool.member_face.startup_parameters() {
            push(&mut canonical, &parameter.name);
            push(&mut canonical, &parameter.value_type);
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
            .member_face
            .inputs()
            .iter()
            .chain(pool.member_face.outputs())
        {
            push(&mut canonical, port.port_id.as_str());
            push(&mut canonical, port.value_kind.as_str());
            push(&mut canonical, port.temporal.as_str());
            push(&mut canonical, &format!("{:?}", port.direction));
        }
        for consumer in &pool.consumers {
            push(&mut canonical, consumer.as_str());
        }
    }
    for row in provenance {
        push(&mut canonical, &row.gear_id);
        push(&mut canonical, &row.form_path.join("/"));
        push(&mut canonical, &row.source_form);
        push(&mut canonical, &row.source_gear);
    }
    ExpandedFormId::from(hash_string(&canonical))
}

pub(super) fn provenance_digest(
    source_document_id: &conduit_core::SourceDocumentId,
    provenance: &[ExpandedGearProvenance],
) -> String {
    let mut canonical = String::from("canonical-expansion-provenance");
    push(&mut canonical, source_document_id.as_str());
    for row in provenance {
        push(&mut canonical, &row.gear_id);
        push(&mut canonical, &row.form_path.join("/"));
        push(&mut canonical, &row.source_form);
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
