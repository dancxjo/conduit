use conduit_core::{port_id, ConfigurationEntry, FormId, KindId, OperationId};
use conduit_signal::{
    pulse_configuration_entries, pulse_kind, pulse_outputs, show_inputs, show_kind,
    signal_value_kind, PulseConfiguration, PULSE_KIND, SHOW_KIND, SIGNAL_PORT,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedOperation {
    pub operation_id: OperationId,
    pub kind_id: KindId,
    pub inputs: Vec<conduit_core::PortDescriptor>,
    pub outputs: Vec<conduit_core::PortDescriptor>,
    pub configuration: Vec<ConfigurationEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedConnection {
    pub source_operation_id: OperationId,
    pub source_port_id: conduit_core::PortId,
    pub sink_operation_id: OperationId,
    pub sink_port_id: conduit_core::PortId,
    pub value_kind: KindId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedForm {
    pub form_id: FormId,
    pub name: String,
    pub operations: Vec<CheckedOperation>,
    pub connections: Vec<CheckedConnection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormError {
    InvalidHeader,
    IncompleteForm,
    InvalidBlockStart,
    MissingBlockEnd,
    EmptyFormName,
    DuplicateOperation(String),
    UnknownOperation(String),
    UnsupportedKind(String),
    InvalidConfiguration(String),
    InvalidConnection(String),
    InvalidStatement(String),
}

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormError::InvalidHeader => {
                write!(f, "expected first non-comment line to be 'form 0'")
            }
            FormError::IncompleteForm => write!(f, "incomplete form"),
            FormError::InvalidBlockStart => write!(f, "expected form block opener like 'name {{'"),
            FormError::MissingBlockEnd => write!(f, "expected closing '}}' at end of form"),
            FormError::EmptyFormName => write!(f, "form name must not be empty"),
            FormError::DuplicateOperation(name) => write!(f, "duplicate operation '{name}'"),
            FormError::UnknownOperation(name) => write!(f, "unknown operation '{name}'"),
            FormError::UnsupportedKind(kind) => write!(
                f,
                "unsupported kind '{kind}'. supported kinds: {PULSE_KIND}, {SHOW_KIND}"
            ),
            FormError::InvalidConfiguration(message) => {
                write!(f, "invalid configuration: {message}")
            }
            FormError::InvalidConnection(message) => write!(f, "invalid connection: {message}"),
            FormError::InvalidStatement(message) => write!(f, "invalid statement: {message}"),
        }
    }
}

impl std::error::Error for FormError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OperationDraft {
    kind: KindId,
    pulse: PulseConfiguration,
}

impl OperationDraft {
    fn new(kind: &str) -> Result<Self, FormError> {
        match kind {
            PULSE_KIND => Ok(Self {
                kind: pulse_kind(),
                pulse: PulseConfiguration {
                    count: 16,
                    period_ms: 250,
                    initial_level: false,
                },
            }),
            SHOW_KIND => Ok(Self {
                kind: show_kind(),
                pulse: PulseConfiguration {
                    count: 16,
                    period_ms: 250,
                    initial_level: false,
                },
            }),
            other => Err(FormError::UnsupportedKind(other.to_string())),
        }
    }
}

pub fn parse(source: &str) -> Result<CheckedForm, FormError> {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    if lines.first().copied().unwrap_or("") != "form 0" {
        return Err(FormError::InvalidHeader);
    }
    if lines.len() < 3 {
        return Err(FormError::IncompleteForm);
    }

    let block_start = lines[1];
    if !block_start.ends_with('{') {
        return Err(FormError::InvalidBlockStart);
    }
    let name = block_start.trim_end_matches('{').trim().to_string();
    if name.is_empty() {
        return Err(FormError::EmptyFormName);
    }
    if lines.last().copied().unwrap_or("") != "}" {
        return Err(FormError::MissingBlockEnd);
    }

    let mut operations = BTreeMap::<String, OperationDraft>::new();
    let mut cords = Vec::<CheckedConnection>::new();

    for raw in &lines[2..lines.len() - 1] {
        let line = raw.trim();
        if let Some((left, right)) = line.split_once(':') {
            let operation_id = left.trim().to_string();
            if operations.contains_key(&operation_id) {
                return Err(FormError::DuplicateOperation(operation_id));
            }
            operations.insert(operation_id, OperationDraft::new(right.trim())?);
            continue;
        }

        if let Some((left, right)) = line.split_once('=') {
            let (operation_id, key) = left
                .trim()
                .split_once('.')
                .ok_or_else(|| FormError::InvalidConfiguration(line.to_string()))?;
            let operation = operations
                .get_mut(operation_id.trim())
                .ok_or_else(|| FormError::UnknownOperation(operation_id.trim().to_string()))?;
            if operation.kind.as_str() != PULSE_KIND {
                return Err(FormError::InvalidConfiguration(format!(
                    "{key} is only valid on {PULSE_KIND}"
                )));
            }
            match key.trim() {
                "count" => {
                    operation.pulse.count = right.trim().parse().map_err(|_| {
                        FormError::InvalidConfiguration(format!("invalid count '{}'", right.trim()))
                    })?
                }
                "period-ms" => {
                    operation.pulse.period_ms = right.trim().parse().map_err(|_| {
                        FormError::InvalidConfiguration(format!(
                            "invalid period-ms '{}'",
                            right.trim()
                        ))
                    })?
                }
                "initial" => {
                    operation.pulse.initial_level = match right.trim() {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(FormError::InvalidConfiguration(format!(
                                "invalid boolean '{other}'"
                            )))
                        }
                    }
                }
                other => {
                    return Err(FormError::InvalidConfiguration(format!(
                        "unsupported pulse key '{other}'"
                    )))
                }
            }
            continue;
        }

        if let Some((left, right)) = line.split_once("->") {
            cords.push(parse_connection(left.trim(), right.trim(), &operations)?);
            continue;
        }

        if let Some((left, right)) = line.split_once('>') {
            let source = left.trim();
            let sink = right.trim();
            let explicit = format!("{source}.{SIGNAL_PORT} -> {sink}.{SIGNAL_PORT}");
            let (lhs, rhs) = explicit
                .split_once("->")
                .expect("shorthand expansion must contain arrow");
            cords.push(parse_connection(lhs.trim(), rhs.trim(), &operations)?);
            continue;
        }

        return Err(FormError::InvalidStatement(line.to_string()));
    }

    let checked_operations = operations
        .into_iter()
        .map(|(operation_name, draft)| match draft.kind.as_str() {
            PULSE_KIND => CheckedOperation {
                operation_id: OperationId::from(operation_name),
                kind_id: pulse_kind(),
                inputs: Vec::new(),
                outputs: pulse_outputs(),
                configuration: pulse_configuration_entries(&draft.pulse),
            },
            SHOW_KIND => CheckedOperation {
                operation_id: OperationId::from(operation_name),
                kind_id: show_kind(),
                inputs: show_inputs(),
                outputs: Vec::new(),
                configuration: Vec::new(),
            },
            _ => unreachable!("kind validation already performed"),
        })
        .collect::<Vec<_>>();

    let form_id = FormId::from(hash_string(&canonical_form_text(
        &name,
        &checked_operations,
        &cords,
    )));
    Ok(CheckedForm {
        form_id,
        name,
        operations: checked_operations,
        connections: cords,
    })
}

fn parse_connection(
    left: &str,
    right: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let (source_operation, source_port) = parse_endpoint(left)?;
    let (sink_operation, sink_port) = parse_endpoint(right)?;
    let source = operations
        .get(source_operation)
        .ok_or_else(|| FormError::UnknownOperation(source_operation.to_string()))?;
    let sink = operations
        .get(sink_operation)
        .ok_or_else(|| FormError::UnknownOperation(sink_operation.to_string()))?;
    if source.kind.as_str() != PULSE_KIND {
        return Err(FormError::InvalidConnection(format!(
            "source '{source_operation}' is not {PULSE_KIND}"
        )));
    }
    if sink.kind.as_str() != SHOW_KIND {
        return Err(FormError::InvalidConnection(format!(
            "sink '{sink_operation}' is not {SHOW_KIND}"
        )));
    }
    if source_port != SIGNAL_PORT || sink_port != SIGNAL_PORT {
        return Err(FormError::InvalidConnection(format!(
            "only the explicit port '{SIGNAL_PORT}' is currently supported"
        )));
    }
    Ok(CheckedConnection {
        source_operation_id: OperationId::from(source_operation),
        source_port_id: port_id(source_port),
        sink_operation_id: OperationId::from(sink_operation),
        sink_port_id: port_id(sink_port),
        value_kind: signal_value_kind(),
    })
}

fn parse_endpoint(endpoint: &str) -> Result<(&str, &str), FormError> {
    endpoint.split_once('.').ok_or_else(|| {
        FormError::InvalidConnection(format!("expected explicit port in '{endpoint}'"))
    })
}

fn canonical_form_text(
    name: &str,
    operations: &[CheckedOperation],
    connections: &[CheckedConnection],
) -> String {
    let mut text = format!("form:{name}\n");
    for operation in operations {
        text.push_str(&format!(
            "op:{}:{}|",
            operation.operation_id.as_str(),
            operation.kind_id.as_str()
        ));
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
    text
}

fn render_value(value: &conduit_core::ConfigurationValue) -> String {
    match value {
        conduit_core::ConfigurationValue::Bool(value) => value.to_string(),
        conduit_core::ConfigurationValue::U64(value) => value.to_string(),
    }
}

fn hash_string(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble out of range"),
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use conduit_signal::{PULSE_KIND, SHOW_KIND};

    #[test]
    fn parses_port_aware_form() {
        let form = parse(
            "form 0\n\nsignal-demo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 3\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n",
        )
        .expect("form should parse");

        assert_eq!(form.operations.len(), 2);
        assert_eq!(form.operations[0].kind_id.as_str(), PULSE_KIND);
        assert_eq!(form.operations[1].kind_id.as_str(), SHOW_KIND);
        assert_eq!(form.connections.len(), 1);
        assert_eq!(form.connections[0].source_port_id.as_str(), "signal");
        assert!(!form.form_id.as_str().is_empty());
    }

    #[test]
    fn rejects_old_show_spelling() {
        let error = parse("form 0\n\nbad {\n    show: display/show\n}\n")
            .expect_err("old spelling must fail");

        assert!(error.to_string().contains("presentation/show"));
    }
}
