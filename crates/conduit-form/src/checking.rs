use super::*;

pub(crate) fn canonical_form_text(
    name: &str,
    operations: &[CheckedOperation],
    connections: &[CheckedConnection],
    exports: &[CheckedExport],
) -> String {
    let mut text = format!("form:{name}\n");
    for operation in operations {
        text.push_str(&format!(
            "op:{}:{}:{}|",
            operation.operation_id.as_str(),
            operation.kind_id.as_str(),
            operation.kind_contract_revision.as_str()
        ));
        for port in operation.inputs.iter().chain(&operation.outputs) {
            let direction = match port.direction {
                conduit_core::PortDirection::Input => "input",
                conduit_core::PortDirection::Output => "output",
            };
            text.push_str(&format!(
                "port:{}:{}:{}|",
                port.port_id.as_str(),
                port.value_kind.as_str(),
                direction
            ));
        }
        for entry in &operation.configuration {
            text.push_str(&format!(
                "cfg:{}={}|",
                entry.key,
                render_value(&entry.value)
            ));
        }
    }
    for connection in connections {
        text.push_str(&format!(
            "conn:{}:{}->{}:{}|",
            connection.source_operation_id.as_str(),
            connection.source_port_id.as_str(),
            connection.sink_operation_id.as_str(),
            connection.sink_port_id.as_str()
        ));
    }
    for export in exports {
        text.push_str(&format!(
            "export:{}:{}|",
            export.capability_id.as_str(),
            export.kind_id.as_str(),
        ));
        for face in export.input_faces.iter().chain(&export.output_faces) {
            let direction = match face.external_port.direction {
                PortDirection::Input => "input",
                PortDirection::Output => "output",
            };
            text.push_str(&format!(
                "face:{direction}:{}:{}={}:{}:terminal-independent|",
                face.external_port.port_id.as_str(),
                face.external_port.value_kind.as_str(),
                face.internal_operation_id.as_str(),
                face.internal_port_id.as_str(),
            ));
        }
    }
    text
}

pub(crate) fn checked_form_id(
    name: &str,
    operations: &[CheckedOperation],
    connections: &[CheckedConnection],
    exports: &[CheckedExport],
) -> CheckedFormId {
    CheckedFormId::from(hash_string(&canonical_form_text(
        name,
        operations,
        connections,
        exports,
    )))
}

pub(crate) fn expanded_form_id(
    checked_form_id: &CheckedFormId,
    nested_forms: &[CheckedNestedForm],
) -> ExpandedFormId {
    let mut canonical = format!("expanded-form:{}", checked_form_id.as_str());
    for nested in nested_forms {
        canonical.push_str("|nested:");
        push_identity_field(&mut canonical, nested.operation_id.as_str());
        push_identity_field(&mut canonical, nested.export_capability_id.as_str());
        push_identity_field(&mut canonical, nested.form.expanded_form_id.as_str());
    }
    ExpandedFormId::from(hash_string(&canonical))
}

pub(crate) fn exported_contract_revision(
    kind_id: &KindId,
    inputs: &[CheckedCompositeFace],
    outputs: &[CheckedCompositeFace],
) -> KindContractRevision {
    let mut canonical = String::from("checked-export-contract:");
    push_identity_field(&mut canonical, kind_id.as_str());
    for (direction, faces) in [("input", inputs), ("output", outputs)] {
        for face in faces {
            push_identity_field(&mut canonical, direction);
            push_identity_field(&mut canonical, face.external_port.port_id.as_str());
            push_identity_field(&mut canonical, face.external_port.value_kind.as_str());
            push_identity_field(
                &mut canonical,
                match face.terminal {
                    CompositeFaceTerminal::Independent => "independent",
                    CompositeFaceTerminal::Coupled => "coupled",
                },
            );
        }
    }
    KindContractRevision::from(format!("checked-export:{}", hash_string(&canonical)))
}

pub(crate) fn push_identity_field(canonical: &mut String, value: &str) {
    canonical.push_str(&value.len().to_string());
    canonical.push(':');
    canonical.push_str(value);
    canonical.push('|');
}

pub(crate) fn render_value(value: &ConfigurationValue) -> String {
    match value {
        ConfigurationValue::Bool(value) => value.to_string(),
        ConfigurationValue::U64(value) => value.to_string(),
    }
}

pub(crate) fn hash_string(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

pub(crate) fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble out of range"),
    }
}
