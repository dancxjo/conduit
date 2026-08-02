use std::fmt::Write as _;

use crate::{
    CompositeDefinition, ExportDirection, InstancePool, Node, PackageImportSelection, Panel,
    PoolAdmission, PoolCleanup, PoolSupervision, PortGroup, PortGroupShape, SourcePressure,
    SourceValue,
};

/// Formats one semantic source AST into the single current concise grammar.
///
/// Lossless round trips remain the responsibility of `SourceDocument`; this
/// formatter deliberately emits canonical source rather than retained trivia.
#[must_use]
pub fn format_panel(panel: &Panel) -> String {
    let mut output = format!("panel {}\n", panel.version);
    for import in &panel.imports {
        write!(output, "\nimport {:?} as {}", import.target, import.alias).unwrap();
        if let Some(hash) = &import.content_hash {
            write!(output, " pin {hash:?}").unwrap();
        }
        output.push('\n');
    }
    for import in &panel.package_imports {
        write!(output, "\nimport {}", import.target).unwrap();
        match &import.selection {
            PackageImportSelection::Named(names) => {
                output.push_str("/{");
                for (index, name) in names.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    output.push_str(&name.export);
                    if name.local != name.export {
                        write!(output, " as {}", name.local).unwrap();
                    }
                }
                output.push('}');
            }
            PackageImportSelection::Alias { local, .. } => {
                write!(output, " as {local}").unwrap();
            }
        }
        output.push('\n');
    }
    for interface in &panel.interfaces {
        write!(output, "\ninterface {} {{\n", interface.id).unwrap();
        for member in &interface.members {
            output.push_str("  ");
            directional(&mut output, member.direction, &member.id);
            write!(output, ": {}", member.port_contract).unwrap();
            if member.optional {
                output.push_str(" optional");
            }
            output.push('\n');
        }
        output.push_str("}\n");
    }
    for definition in &panel.definitions {
        format_definition(&mut output, definition);
    }
    if !panel.nodes.is_empty() || !panel.cords.is_empty() {
        output.push('\n');
    }
    for node in &panel.nodes {
        if node.expression.is_none() {
            format_node(&mut output, node, "");
        }
    }
    for cord in &panel.cords {
        format_cord(&mut output, cord, &panel.nodes, "");
    }
    for root in &panel.roots {
        writeln!(output, "root {}", root.target).unwrap();
    }
    for group in &panel.port_groups {
        format_group(&mut output, group, "");
    }
    for pool in &panel.pools {
        format_pool(&mut output, pool, "");
    }
    for supervision in &panel.supervisions {
        writeln!(
            output,
            "supervise {} with {}",
            supervision.subject, supervision.handler
        )
        .unwrap();
    }
    output
}

fn format_definition(output: &mut String, definition: &CompositeDefinition) {
    write!(output, "\n{}", definition.id).unwrap();
    if !definition.parameters.is_empty() {
        output.push('(');
        for (index, parameter) in definition.parameters.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            write!(output, "{}: {}", parameter.id, parameter.value_type).unwrap();
            if let Some(default) = &parameter.default {
                output.push_str(" = ");
                format_value(output, default);
            }
        }
        output.push(')');
    }
    format_claims(output, &definition.implements);
    output.push_str(" {\n");
    for node in &definition.nodes {
        if node.expression.is_none() {
            format_node(output, node, "  ");
        }
    }
    for cord in &definition.cords {
        format_cord(output, cord, &definition.nodes, "  ");
    }
    for export in &definition.exports {
        output.push_str("  export ");
        directional(output, export.direction, &export.id);
        writeln!(
            output,
            " = {}",
            endpoint_text(&export.target.node, &export.target.port)
        )
        .unwrap();
    }
    for binding in &definition.bindings {
        writeln!(
            output,
            "  bind {} = {}",
            binding.parameter,
            endpoint_text(&binding.target.node, &binding.target.port)
        )
        .unwrap();
    }
    for group in &definition.port_groups {
        format_group(output, group, "  ");
    }
    for pool in &definition.pools {
        format_pool(output, pool, "  ");
    }
    for supervision in &definition.supervisions {
        writeln!(
            output,
            "  supervise {} with {}",
            supervision.subject, supervision.handler
        )
        .unwrap();
    }
    output.push_str("}\n");
}

fn format_node(output: &mut String, node: &Node, indent: &str) {
    if node.kind == "std/literal"
        && node.constraint.is_none()
        && node.implements.is_empty()
        && node.config.len() == 1
        && node.config[0].key == "value"
    {
        write!(output, "{indent}{} = ", node.id).unwrap();
        format_value(output, &node.config[0].value);
        output.push('\n');
        return;
    }
    write!(output, "{indent}{}: {}", node.id, node.kind).unwrap();
    if let Some(constraint) = &node.constraint {
        write!(output, " using {constraint}").unwrap();
    }
    format_claims(output, &node.implements);
    if node.config.is_empty() {
        output.push('\n');
        return;
    }
    output.push_str(" {\n");
    for entry in &node.config {
        write!(output, "{indent}  {} = ", entry.key).unwrap();
        format_value(output, &entry.value);
        output.push('\n');
    }
    writeln!(output, "{indent}}}").unwrap();
}

fn format_claims(output: &mut String, claims: &[crate::InterfaceClaim]) {
    if claims.is_empty() {
        return;
    }
    output.push_str(" implements ");
    for (index, claim) in claims.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str(&claim.interface);
    }
}

fn format_cord(output: &mut String, cord: &crate::Cord, nodes: &[Node], indent: &str) {
    let to = nodes
        .iter()
        .find(|node| node.id == cord.to.node)
        .and_then(|node| {
            node.expression.as_ref().map(|expression| {
                let stage = match node.kind.as_str() {
                    "std/flow/keep" => "keep",
                    "std/flow/map" => "map",
                    "std/flow/stop" => "stop",
                    kind => kind,
                };
                let mut text = format!("{stage} {{ ");
                format_expression(&mut text, expression);
                text.push_str(" }");
                text
            })
        })
        .unwrap_or_else(|| endpoint_text(&cord.to.node, &cord.to.port));
    writeln!(
        output,
        "{indent}{} > {} {{",
        endpoint_text(&cord.from.node, &cord.from.port),
        to
    )
    .unwrap();
    writeln!(output, "{indent}  capacity = {}", cord.capacity_items).unwrap();
    writeln!(
        output,
        "{indent}  max_value_bytes = {}",
        cord.max_value_bytes
    )
    .unwrap();
    writeln!(
        output,
        "{indent}  max_queued_bytes = {}",
        cord.max_queued_bytes
    )
    .unwrap();
    writeln!(
        output,
        "{indent}  low_watermark = {}",
        cord.low_watermark_items
    )
    .unwrap();
    writeln!(
        output,
        "{indent}  high_watermark = {}",
        cord.high_watermark_items
    )
    .unwrap();
    match &cord.pressure {
        SourcePressure::Block => writeln!(output, "{indent}  pressure = block").unwrap(),
        SourcePressure::Reject => writeln!(output, "{indent}  pressure = reject").unwrap(),
        SourcePressure::Coalesce { relation } => {
            writeln!(output, "{indent}  pressure = coalesce").unwrap();
            writeln!(output, "{indent}  coalescer = {relation}").unwrap();
        }
        SourcePressure::Sample { every, offset } => {
            writeln!(output, "{indent}  pressure = sample").unwrap();
            writeln!(output, "{indent}  sample_every = {every}").unwrap();
            writeln!(output, "{indent}  sample_offset = {offset}").unwrap();
        }
        SourcePressure::DropDisposable => {
            writeln!(output, "{indent}  pressure = drop-disposable").unwrap();
        }
        SourcePressure::Disconnect => {
            writeln!(output, "{indent}  pressure = disconnect").unwrap();
        }
        SourcePressure::Fail => writeln!(output, "{indent}  pressure = fail").unwrap(),
    }
    writeln!(output, "{indent}}}").unwrap();
}

fn format_expression(output: &mut String, expression: &crate::SourceExpression) {
    match expression {
        crate::SourceExpression::Value(value) => format_value(output, value),
        crate::SourceExpression::Binding(binding) => output.push_str(binding),
        crate::SourceExpression::Binary {
            operation,
            left,
            right,
            operator_span: _,
        } => {
            output.push('(');
            format_expression(output, left);
            write!(
                output,
                " {} ",
                match operation {
                    crate::ExpressionOperator::Add => "+",
                    crate::ExpressionOperator::Subtract => "-",
                    crate::ExpressionOperator::Multiply => "*",
                    crate::ExpressionOperator::Divide => "/",
                    crate::ExpressionOperator::LessThan => "<",
                    crate::ExpressionOperator::LessThanOrEqual => "<=",
                    crate::ExpressionOperator::GreaterThan => ">",
                    crate::ExpressionOperator::GreaterThanOrEqual => ">=",
                    crate::ExpressionOperator::Equal => "==",
                    crate::ExpressionOperator::NotEqual => "!=",
                }
            )
            .unwrap();
            format_expression(output, right);
            output.push(')');
        }
    }
}

fn format_group(output: &mut String, group: &PortGroup, indent: &str) {
    output.push_str(indent);
    output.push_str("port-group ");
    directional(output, group.direction, &group.id);
    write!(output, ": {} ", group.port_contract).unwrap();
    match &group.shape {
        PortGroupShape::Indexed => writeln!(output, "indexed max {}", group.maximum).unwrap(),
        PortGroupShape::Keyed(members) => {
            writeln!(output, "keyed max {} {{", group.maximum).unwrap();
            for member in members {
                writeln!(output, "{indent}  member {}", member.key).unwrap();
            }
            writeln!(output, "{indent}}}").unwrap();
        }
    }
}

fn format_pool(output: &mut String, pool: &InstancePool, indent: &str) {
    writeln!(output, "{indent}pool {}: {} {{", pool.id, pool.template).unwrap();
    writeln!(output, "{indent}  maximum = {}", pool.maximum).unwrap();
    match pool.admission {
        PoolAdmission::Reject => writeln!(output, "{indent}  admission = reject").unwrap(),
        PoolAdmission::Block => writeln!(output, "{indent}  admission = block").unwrap(),
        PoolAdmission::QueueBounded(maximum) => {
            writeln!(output, "{indent}  admission = queue_bounded").unwrap();
            writeln!(output, "{indent}  admission_queue = {maximum}").unwrap();
        }
        PoolAdmission::Fail => writeln!(output, "{indent}  admission = fail").unwrap(),
    }
    writeln!(output, "{indent}  deadline_ms = {}", pool.deadline_ms).unwrap();
    writeln!(
        output,
        "{indent}  idle_timeout_ms = {}",
        pool.idle_timeout_ms
    )
    .unwrap();
    match &pool.supervision {
        PoolSupervision::FailTogether => {
            writeln!(output, "{indent}  supervision = fail_together").unwrap();
        }
        PoolSupervision::Isolate => {
            writeln!(output, "{indent}  supervision = isolate").unwrap();
        }
        PoolSupervision::RestartBounded {
            attempts,
            backoff_ms,
        } => {
            writeln!(output, "{indent}  supervision = restart_bounded").unwrap();
            writeln!(output, "{indent}  restart_attempts = {attempts}").unwrap();
            writeln!(output, "{indent}  restart_backoff_ms = {backoff_ms}").unwrap();
        }
        PoolSupervision::Fallback(target) => {
            writeln!(output, "{indent}  supervision = fallback").unwrap();
            writeln!(output, "{indent}  fallback = {target}").unwrap();
        }
        PoolSupervision::Escalate => {
            writeln!(output, "{indent}  supervision = escalate").unwrap();
        }
    }
    writeln!(
        output,
        "{indent}  cleanup = {}",
        match pool.cleanup {
            PoolCleanup::Drain => "drain",
            PoolCleanup::Abort => "abort",
        }
    )
    .unwrap();
    writeln!(output, "{indent}}}").unwrap();
}

fn directional(output: &mut String, direction: ExportDirection, id: &str) {
    match direction {
        ExportDirection::Input => write!(output, "> {id}").unwrap(),
        ExportDirection::Output => write!(output, "{id} >").unwrap(),
    }
}

fn endpoint_text(node: &str, port: &str) -> String {
    if port.is_empty() {
        node.to_owned()
    } else {
        format!("{node}.{port}")
    }
}

fn format_value(output: &mut String, value: &SourceValue) {
    match value {
        SourceValue::Boolean(value) => write!(output, "{value}").unwrap(),
        SourceValue::Integer(value) => write!(output, "{value}").unwrap(),
        SourceValue::Text(value) => write!(output, "{value:?}").unwrap(),
        SourceValue::Bytes(value) => {
            output.push_str("bytes(\"");
            for byte in value {
                write!(output, "{byte:02x}").unwrap();
            }
            output.push_str("\")");
        }
        SourceValue::Reference(value) => write!(output, "ref({value:?})").unwrap(),
        SourceValue::ContractReference(value) => write!(output, "contract({value:?})").unwrap(),
        SourceValue::SecretReference(value) => write!(output, "secret({value:?})").unwrap(),
        SourceValue::ExactDecimal(value) => write!(output, "decimal({value:?})").unwrap(),
        SourceValue::List(values) => {
            output.push_str("list(");
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                format_value(output, value);
            }
            output.push(')');
        }
        SourceValue::Record(fields) => {
            output.push_str("record(");
            for (index, (key, value)) in fields.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                write!(output, "{key}=").unwrap();
                format_value(output, value);
            }
            output.push(')');
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{format_panel, parse, semantic_source_hash};

    #[test]
    fn canonical_format_uses_only_the_concise_current_surface() {
        let source = "panel 0\nmessage = \"hello\"\nsink: display/text\nmessage > sink\n";
        let panel = parse(source).unwrap();
        let formatted = format_panel(&panel);
        assert!(formatted.contains("message = \"hello\""));
        assert!(formatted.contains("message > sink"));
        assert!(!formatted.contains("node message"));
        assert!(!formatted.contains("cord message"));
        assert!(!formatted.contains("->"));
        let reparsed = parse(&formatted).unwrap();
        assert_eq!(
            semantic_source_hash(&panel),
            semantic_source_hash(&reparsed)
        );
    }

    #[test]
    fn expression_chain_formatting_preserves_graph_and_expression_identity() {
        let source = "panel 0\nages: fixture/source\nadults: fixture/sink\n\
                      ages > keep { it > 18 } > adults\n";
        let panel = parse(source).unwrap();
        let formatted = format_panel(&panel);

        assert!(formatted.contains("ages > keep { (it > 18) }"));
        assert!(!formatted.contains("node "));
        assert!(!formatted.contains("cord "));
        assert!(!formatted.contains("->"));
        let reparsed = parse(&formatted).unwrap();
        assert_eq!(
            semantic_source_hash(&panel),
            semantic_source_hash(&reparsed)
        );
    }
}
