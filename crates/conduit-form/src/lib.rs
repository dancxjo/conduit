use conduit_core::{
    ConfigurationEntry, ConfigurationValue, FormId, KindId, OperationId, PortDescriptor, PortId,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedOperation {
    pub operation_id: OperationId,
    pub kind_id: KindId,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<ConfigurationEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedConnection {
    pub source_operation_id: OperationId,
    pub source_port_id: PortId,
    pub sink_operation_id: OperationId,
    pub sink_port_id: PortId,
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
pub struct ConfigurationField {
    pub key: String,
    pub default_value: ConfigurationValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindDefinition {
    pub kind_id: KindId,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<ConfigurationField>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileCatalog {
    kinds: BTreeMap<KindId, KindDefinition>,
}

impl ProfileCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, definition: KindDefinition) -> Result<(), FormError> {
        if self.kinds.contains_key(&definition.kind_id) {
            return Err(FormError::DuplicateKind(
                definition.kind_id.as_str().to_string(),
            ));
        }
        self.kinds.insert(definition.kind_id.clone(), definition);
        Ok(())
    }

    pub fn get(&self, kind_id: &KindId) -> Option<&KindDefinition> {
        self.kinds.get(kind_id)
    }

    fn supported_kinds(&self) -> String {
        self.kinds
            .keys()
            .map(|kind| kind.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormError {
    InvalidHeader,
    IncompleteForm,
    InvalidBlockStart,
    MissingBlockEnd,
    EmptyFormName,
    DuplicateKind(String),
    DuplicateOperation(String),
    UnknownOperation(String),
    UnsupportedKind { kind: String, supported: String },
    InvalidConfiguration(String),
    InvalidConnection(String),
    InvalidStatement(String),
}

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHeader => write!(f, "expected first non-comment line to be 'form 0'"),
            Self::IncompleteForm => write!(f, "incomplete form"),
            Self::InvalidBlockStart => write!(f, "expected form block opener like 'name {{'"),
            Self::MissingBlockEnd => write!(f, "expected closing '}}' at end of form"),
            Self::EmptyFormName => write!(f, "form name must not be empty"),
            Self::DuplicateKind(kind) => write!(f, "duplicate profile kind '{kind}'"),
            Self::DuplicateOperation(name) => write!(f, "duplicate operation '{name}'"),
            Self::UnknownOperation(name) => write!(f, "unknown operation '{name}'"),
            Self::UnsupportedKind { kind, supported } => {
                write!(f, "unsupported kind '{kind}'. supported kinds: {supported}")
            }
            Self::InvalidConfiguration(message) => {
                write!(f, "invalid configuration: {message}")
            }
            Self::InvalidConnection(message) => write!(f, "invalid connection: {message}"),
            Self::InvalidStatement(message) => write!(f, "invalid statement: {message}"),
        }
    }
}

impl std::error::Error for FormError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OperationDraft {
    definition: KindDefinition,
    configuration: Vec<ConfigurationEntry>,
}

impl OperationDraft {
    fn new(kind: &str, catalog: &ProfileCatalog) -> Result<Self, FormError> {
        let kind_id = KindId::from(kind);
        let definition =
            catalog
                .get(&kind_id)
                .cloned()
                .ok_or_else(|| FormError::UnsupportedKind {
                    kind: kind.to_string(),
                    supported: catalog.supported_kinds(),
                })?;
        let configuration = definition
            .configuration
            .iter()
            .map(|field| ConfigurationEntry {
                key: field.key.clone(),
                value: field.default_value.clone(),
            })
            .collect();
        Ok(Self {
            definition,
            configuration,
        })
    }
}

pub fn parse(source: &str, catalog: &ProfileCatalog) -> Result<CheckedForm, FormError> {
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
    let mut connections = Vec::<CheckedConnection>::new();
    for raw in &lines[2..lines.len() - 1] {
        let line = raw.trim();
        if let Some((left, right)) = line.split_once(':') {
            let operation_id = left.trim().to_string();
            if operations.contains_key(&operation_id) {
                return Err(FormError::DuplicateOperation(operation_id));
            }
            operations.insert(operation_id, OperationDraft::new(right.trim(), catalog)?);
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
            let entry = operation
                .configuration
                .iter_mut()
                .find(|entry| entry.key == key.trim())
                .ok_or_else(|| {
                    FormError::InvalidConfiguration(format!(
                        "unsupported key '{}' for '{}'",
                        key.trim(),
                        operation.definition.kind_id.as_str()
                    ))
                })?;
            entry.value = parse_configuration_value(right.trim(), &entry.value)?;
            continue;
        }
        if let Some((left, right)) = line.split_once("->") {
            connections.push(parse_connection(left.trim(), right.trim(), &operations)?);
            continue;
        }
        if let Some((left, right)) = line.split_once('>') {
            connections.push(parse_shorthand_connection(
                left.trim(),
                right.trim(),
                &operations,
            )?);
            continue;
        }
        return Err(FormError::InvalidStatement(line.to_string()));
    }

    let checked_operations = operations
        .iter()
        .map(|(operation_name, draft)| CheckedOperation {
            operation_id: OperationId::from(operation_name.as_str()),
            kind_id: draft.definition.kind_id.clone(),
            inputs: draft.definition.inputs.clone(),
            outputs: draft.definition.outputs.clone(),
            configuration: draft.configuration.clone(),
        })
        .collect::<Vec<_>>();
    let form_id = FormId::from(hash_string(&canonical_form_text(
        &name,
        &checked_operations,
        &connections,
    )));
    Ok(CheckedForm {
        form_id,
        name,
        operations: checked_operations,
        connections,
    })
}

fn parse_configuration_value(
    source: &str,
    expected: &ConfigurationValue,
) -> Result<ConfigurationValue, FormError> {
    match expected {
        ConfigurationValue::Bool(_) => match source {
            "true" => Ok(ConfigurationValue::Bool(true)),
            "false" => Ok(ConfigurationValue::Bool(false)),
            _ => Err(FormError::InvalidConfiguration(format!(
                "invalid boolean '{source}'"
            ))),
        },
        ConfigurationValue::U64(_) => source
            .parse()
            .map(ConfigurationValue::U64)
            .map_err(|_| FormError::InvalidConfiguration(format!("invalid integer '{source}'"))),
    }
}

fn parse_connection(
    left: &str,
    right: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let (source_operation, source_port) = parse_endpoint(left)?;
    let (sink_operation, sink_port) = parse_endpoint(right)?;
    connection_from_ports(
        source_operation,
        source_port,
        sink_operation,
        sink_port,
        operations,
    )
}

fn parse_shorthand_connection(
    source_operation: &str,
    sink_operation: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let source = operation(operations, source_operation)?;
    let sink = operation(operations, sink_operation)?;
    if source.definition.outputs.len() != 1 || sink.definition.inputs.len() != 1 {
        return Err(FormError::InvalidConnection(format!(
            "shorthand requires exactly one output and one input for '{source_operation} > {sink_operation}'"
        )));
    }
    connection_from_ports(
        source_operation,
        source.definition.outputs[0].port_id.as_str(),
        sink_operation,
        sink.definition.inputs[0].port_id.as_str(),
        operations,
    )
}

fn connection_from_ports(
    source_operation: &str,
    source_port: &str,
    sink_operation: &str,
    sink_port: &str,
    operations: &BTreeMap<String, OperationDraft>,
) -> Result<CheckedConnection, FormError> {
    let source = operation(operations, source_operation)?;
    let sink = operation(operations, sink_operation)?;
    let source_descriptor = source
        .definition
        .outputs
        .iter()
        .find(|port| port.port_id.as_str() == source_port)
        .ok_or_else(|| {
            FormError::InvalidConnection(format!(
                "'{source_operation}' has no output port '{source_port}'"
            ))
        })?;
    let sink_descriptor = sink
        .definition
        .inputs
        .iter()
        .find(|port| port.port_id.as_str() == sink_port)
        .ok_or_else(|| {
            FormError::InvalidConnection(format!(
                "'{sink_operation}' has no input port '{sink_port}'"
            ))
        })?;
    if source_descriptor.value_kind != sink_descriptor.value_kind {
        return Err(FormError::InvalidConnection(format!(
            "value kind '{}' cannot connect to '{}'",
            source_descriptor.value_kind.as_str(),
            sink_descriptor.value_kind.as_str()
        )));
    }
    Ok(CheckedConnection {
        source_operation_id: OperationId::from(source_operation),
        source_port_id: source_descriptor.port_id.clone(),
        sink_operation_id: OperationId::from(sink_operation),
        sink_port_id: sink_descriptor.port_id.clone(),
        value_kind: source_descriptor.value_kind.clone(),
    })
}

fn operation<'a>(
    operations: &'a BTreeMap<String, OperationDraft>,
    operation_id: &str,
) -> Result<&'a OperationDraft, FormError> {
    operations
        .get(operation_id)
        .ok_or_else(|| FormError::UnknownOperation(operation_id.to_string()))
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

fn render_value(value: &ConfigurationValue) -> String {
    match value {
        ConfigurationValue::Bool(value) => value.to_string(),
        ConfigurationValue::U64(value) => value.to_string(),
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
    use super::{parse, ConfigurationField, KindDefinition, ProfileCatalog};
    use conduit_core::{kind_id, port_id, ConfigurationValue, PortDescriptor, PortDirection};

    fn catalog() -> ProfileCatalog {
        let value_kind = kind_id("test/value");
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(KindDefinition {
                kind_id: kind_id("test/source"),
                inputs: Vec::new(),
                outputs: vec![PortDescriptor {
                    port_id: port_id("out"),
                    value_kind: value_kind.clone(),
                    direction: PortDirection::Output,
                }],
                configuration: vec![ConfigurationField {
                    key: "count".to_string(),
                    default_value: ConfigurationValue::U64(1),
                }],
            })
            .expect("source kind installs");
        catalog
            .insert(KindDefinition {
                kind_id: kind_id("test/sink"),
                inputs: vec![PortDescriptor {
                    port_id: port_id("in"),
                    value_kind,
                    direction: PortDirection::Input,
                }],
                outputs: Vec::new(),
                configuration: Vec::new(),
            })
            .expect("sink kind installs");
        catalog
    }

    #[test]
    fn parses_catalog_supplied_kinds_and_ports() {
        let form = parse(
            "form 0\n\ndemo {\n source: test/source\n sink: test/sink\n source.count = 3\n source > sink\n}\n",
            &catalog(),
        )
        .expect("form parses");
        assert_eq!(form.operations[0].kind_id.as_str(), "test/sink");
        assert_eq!(form.operations[1].kind_id.as_str(), "test/source");
        assert_eq!(form.connections[0].source_port_id.as_str(), "out");
        assert_eq!(form.connections[0].sink_port_id.as_str(), "in");
    }

    #[test]
    fn rejects_kinds_absent_from_catalog() {
        let error = parse("form 0\n\nbad {\n op: missing/kind\n}\n", &catalog())
            .expect_err("unknown kind fails");
        assert!(error.to_string().contains("missing/kind"));
    }
}
