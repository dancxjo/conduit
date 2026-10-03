//! Narrow immutable Back lowering for this pack, never a speech algorithm.
//! Only checked integer/Boolean/text expression trees are eligible. Unsupported
//! operations refuse preparation rather than falling back to native policy.
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};
use conduit_plot::{
    BinaryOperator as B, PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram, PortableExpressionProjection as Projection, UnaryOperator,
};

pub fn ty(
    value: &StructuredInfoType,
    types: &[(StructuredInfoType, String)],
) -> Result<String, String> {
    if let Some((_, name)) = types.iter().find(|(known, _)| known == value) {
        return Ok(name.clone());
    }
    Ok(match value.shape() {
        StructuredInfoTypeShape::Leaf(kind) => match kind.as_str() {
            "value/i32" => "i32",
            "value/i64" => "i64",
            "value/u32" => "u32",
            "value/u64" => "u64",
            "value/bool" => "bool",
            "value/text" => "&str",
            other => return Err(format!("unsupported leaf {other}")),
        }
        .into(),
        StructuredInfoTypeShape::Record { fields, .. } => {
            let fields = fields
                .iter()
                .map(|field| ty(field.value_type(), types))
                .collect::<Result<Vec<_>, _>>()?;
            format!("({},)", fields.join(","))
        }
        StructuredInfoTypeShape::Nominal { representation, .. } => ty(representation, types)?,
        other => return Err(format!("unsupported native shape {other:?}")),
    })
}

pub fn function(
    name: &str,
    program: &PortableExpressionProgram,
    types: &[(StructuredInfoType, String)],
    inline: bool,
) -> Result<String, String> {
    let attribute = if inline && context_only(&program.root) {
        "#[inline(always)]\n"
    } else {
        ""
    };
    Ok(format!(
        "{attribute}pub fn {name}(input: {}) -> Option<{}> {{ let _ = input; Some({}) }}\n",
        ty(&program.input_type, types)?,
        ty(&program.output_type, types)?,
        node(&program.root, types)?
    ))
}

/// Only authored literal records can become compile-time profile constants.
/// Runtime inputs and arithmetic do not become an unchecked constant shortcut.
pub fn constant(
    program: &PortableExpressionProgram,
    types: &[(StructuredInfoType, String)],
) -> Result<String, String> {
    fn closed(value: &Node) -> bool {
        match &value.operation {
            Op::Literal(_) => true,
            Op::Record(fields) => fields.iter().all(|(_, field)| closed(field)),
            _ => false,
        }
    }
    if !closed(&program.root) {
        return Err("constant requires a closed literal tree".into());
    }
    node(&program.root, types)
}

fn node(value: &Node, types: &[(StructuredInfoType, String)]) -> Result<String, String> {
    Ok(match &value.operation {
        Op::Input => "input".into(),
        Op::Literal(text) => {
            let kind = ty(&value.value_type, types)?;
            if types.iter().any(|(_, name)| name == &kind) {
                format!(
                    "{kind}::r#{}",
                    text.rsplit('.').next().ok_or("variant spelling")?
                )
            } else if kind == "&str" || kind == "bool" {
                text.clone()
            } else {
                format!("({text}_{kind})")
            }
        }
        Op::Projection {
            value,
            member: Projection::Field(name),
        } => {
            let mut shape = value.value_type.shape();
            while let StructuredInfoTypeShape::Nominal { representation, .. } = shape {
                shape = representation.shape();
            }
            let StructuredInfoTypeShape::Record { fields, .. } = shape else {
                return Err("projection requires record".into());
            };
            let ordinal = fields
                .iter()
                .position(|field| field.name() == name)
                .ok_or("projection field absent")?;
            if types.iter().any(|(known, _)| known == &value.value_type) {
                format!("({}).{}", node(value, types)?, name.replace('-', "_"))
            } else {
                format!("({}).{ordinal}", node(value, types)?)
            }
        }
        Op::Unary { operator, operand } => match operator {
            UnaryOperator::Negate => format!("({}).checked_neg()?", node(operand, types)?),
            UnaryOperator::Not => format!("(!({}))", node(operand, types)?),
        },
        Op::Binary {
            operator,
            left,
            right,
            ..
        } => {
            let left = node(left, types)?;
            let right = node(right, types)?;
            let method = match operator {
                B::Add => Some("checked_add"),
                B::Subtract => Some("checked_sub"),
                B::Multiply => Some("checked_mul"),
                B::Divide => Some("checked_div"),
                B::Remainder => Some("checked_rem"),
                _ => None,
            };
            if let Some(method) = method {
                format!("({left}).{method}({right})?")
            } else {
                let op = match operator {
                    B::Equal => "==",
                    B::NotEqual => "!=",
                    B::Less => "<",
                    B::LessOrEqual => "<=",
                    B::Greater => ">",
                    B::GreaterOrEqual => ">=",
                    B::BooleanAnd | B::BitAnd => "&",
                    B::BooleanOr | B::BitOr => "|",
                    B::BitXor => "^",
                    _ => return Err("unsupported operator".into()),
                };
                format!("(({left}) {op} ({right}))")
            }
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => format!(
            "(if {} {{ {} }} else {{ {} }})",
            node(condition, types)?,
            node(when_true, types)?,
            node(when_false, types)?
        ),
        Op::Record(fields) => {
            // Canonical field order, not source spelling, defines representation.
            let StructuredInfoTypeShape::Record {
                fields: field_types,
                ..
            } = value.value_type.shape()
            else {
                return Err("record construction requires structural output".into());
            };
            let values = field_types
                .iter()
                .map(|field| {
                    let (_, expression) = fields
                        .iter()
                        .find(|(name, _)| name == field.name())
                        .ok_or("record field absent")?;
                    node(expression, types)
                })
                .collect::<Result<Vec<_>, String>>()?;
            if let Some((_, name)) = types.iter().find(|(known, _)| known == &value.value_type) {
                let entries = field_types
                    .iter()
                    .zip(values)
                    .map(|(field, value)| {
                        let name = field.name().replace('-', "_");
                        if name == value {
                            name
                        } else {
                            format!("{name}: {value}")
                        }
                    })
                    .collect::<Vec<_>>();
                format!("{name} {{ {} }}", entries.join(","))
            } else {
                format!("({},)", values.join(","))
            }
        }
        Op::Variant { tag, payload } => format!(
            "{}::r#{}({})",
            ty(&value.value_type, types)?,
            tag,
            node(payload, types)?
        ),
        Op::SemanticCall { kind, arguments } if kind == "variant/is" => {
            let [subject, tag] = arguments.as_slice() else {
                return Err("variant/is arity".into());
            };
            let Op::Literal(tag) = &tag.operation else {
                return Err("variant/is tag".into());
            };
            let tag = tag.trim_matches('"');
            let StructuredInfoTypeShape::Variant { cases, .. } = subject.value_type.shape() else {
                return Err("variant/is subject".into());
            };
            let case = cases
                .iter()
                .find(|case| case.tag() == tag)
                .ok_or("variant/is case")?;
            let pattern = if matches!(case.payload_type().shape(),StructuredInfoTypeShape::Leaf(kind) if kind.as_str()==conduit_core::UNIT_INFO_ID)
            {
                ""
            } else {
                "(_)"
            };
            format!(
                "matches!({}, {}::r#{}{pattern})",
                node(subject, types)?,
                ty(&subject.value_type, types)?,
                tag
            )
        }
        other => return Err(format!("unsupported operation {other:?}")),
    })
}

// Preparation-only code-generation hint. Numeric equations keep the compiler's
// normal sharing decisions; projections/selectors can erase carrier copies.
fn context_only(value: &Node) -> bool {
    context_node(value, &mut 128)
}

fn context_node(value: &Node, remaining: &mut usize) -> bool {
    if *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    match &value.operation {
        Op::Input | Op::Literal(_) => true,
        Op::Projection { value, .. } => context_node(value, remaining),
        Op::Unary {
            operator: UnaryOperator::Not,
            operand,
        } => context_node(operand, remaining),
        Op::Binary {
            operator:
                B::Equal
                | B::NotEqual
                | B::Less
                | B::LessOrEqual
                | B::Greater
                | B::GreaterOrEqual
                | B::BooleanAnd
                | B::BooleanOr,
            left,
            right,
            ..
        } => context_node(left, remaining) && context_node(right, remaining),
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            context_node(condition, remaining)
                && context_node(when_true, remaining)
                && context_node(when_false, remaining)
        }
        Op::Record(fields) => fields
            .iter()
            .all(|(_, value)| context_node(value, remaining)),
        Op::Variant { payload, .. } => context_node(payload, remaining),
        Op::SemanticCall { kind, arguments } if kind == "variant/is" => {
            arguments.iter().all(|value| context_node(value, remaining))
        }
        _ => false,
    }
}
