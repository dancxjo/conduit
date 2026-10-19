//! Prepared exact U64/Bool operations keep computed values in native registers.
//! Canonical bytes are decoded only for literals during preparation and at Host
//! boundaries by the caller. Checked/proven arithmetic and lazy branches retain
//! the ordinary prepared evaluator's exact selected behavior.
use super::*;
const MAX_NODES: usize = 128;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Word,
    Bool,
}
#[derive(Clone, Copy)]
enum Value {
    Word(u64),
    Bool(bool),
}
impl Value {
    fn word(self) -> Result<u64, Refusal> {
        match self {
            Self::Word(v) => Ok(v),
            _ => Err(Refusal::InvalidProgram),
        }
    }
    fn boolean(self) -> Result<bool, Refusal> {
        match self {
            Self::Bool(v) => Ok(v),
            _ => Err(Refusal::InvalidProgram),
        }
    }
    fn ordered(self) -> u64 {
        match self {
            Self::Word(v) => v,
            Self::Bool(v) => u64::from(v),
        }
    }
}
struct Node {
    kind: Kind,
    operation: Operation,
}
enum Operation {
    Input,
    Literal(Value),
    Unary(UnaryOperator, Box<Node>),
    Binary(BinaryOperator, bool, Box<Node>, Box<Node>),
    Conditional(Box<Node>, Box<Node>, Box<Node>),
}
pub struct PreparedNativeU64Expression {
    root: Node,
    nodes: usize,
}
impl PreparedNativeU64Expression {
    pub fn new(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        let u64_type = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64"))
            .map_err(|_| Refusal::InvalidProgram)?;
        if program.input_type != u64_type
            || program.output_type != u64_type
            || program.input_maximum_bytes.is_some_and(|n| n < 8)
        {
            return Err(unsupported());
        }
        let prepared = PreparedPortableExpressionEvaluator::new(program)?;
        let PreparedRoot::Primitive {
            node,
            nominal_type: None,
        } = &prepared.root
        else {
            return Err(unsupported());
        };
        let mut nodes = 0;
        let root = lower(node, &mut nodes)?;
        if root.kind != Kind::Word {
            return Err(Refusal::InvalidProgram);
        }
        Ok(Self { root, nodes })
    }
    pub fn evaluate(&self, input: u64) -> Result<u64, Refusal> {
        self.root.evaluate(input)?.word()
    }
    pub fn node_count(&self) -> usize {
        self.nodes
    }
    /// Requested payload storage retained in prepared child-node boxes.
    pub fn retained_storage_bytes(&self) -> usize {
        (self.nodes - 1) * core::mem::size_of::<Node>()
    }
}
fn unsupported() -> Refusal {
    Refusal::UnsupportedType("native-u64/checked-primitive-profile@1".into())
}
fn lower(node: &PreparedNode, nodes: &mut usize) -> Result<Node, Refusal> {
    *nodes += 1;
    if *nodes > MAX_NODES {
        return Err(unsupported());
    }
    let kind = match (node.kind, node.info_kind.as_str()) {
        (PrimitiveInfoKind::U64, "value/u64") => Kind::Word,
        (PrimitiveInfoKind::Bool, "value/bool") => Kind::Bool,
        _ => return Err(unsupported()),
    };
    let operation = match &node.operation {
        PreparedOperation::Input if kind == Kind::Word => Operation::Input,
        PreparedOperation::Literal(bytes) => Operation::Literal(match kind {
            Kind::Word => Value::Word(u64::from_le_bytes(
                bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| Refusal::InvalidProgram)?,
            )),
            Kind::Bool => Value::Bool(
                conduit_core::InfoBool::decode(bytes)
                    .map_err(|_| Refusal::InvalidProgram)?
                    .get(),
            ),
        }),
        PreparedOperation::Unary { operator, operand } => {
            let operand = Box::new(lower(operand, nodes)?);
            if !matches!(
                (operator, kind, operand.kind),
                (UnaryOperator::Not, Kind::Bool, Kind::Bool)
                    | (UnaryOperator::Negate, Kind::Word, Kind::Word)
            ) {
                return Err(Refusal::InvalidProgram);
            }
            Operation::Unary(*operator, operand)
        }
        PreparedOperation::Binary {
            operator,
            proven,
            left,
            right,
        } => {
            let left = Box::new(lower(left, nodes)?);
            let right = Box::new(lower(right, nodes)?);
            if left.kind != right.kind {
                return Err(Refusal::InvalidProgram);
            }
            use BinaryOperator::*;
            let expected = match operator {
                Equal | NotEqual | Less | LessOrEqual | Greater | GreaterOrEqual => Kind::Bool,
                BooleanAnd | BooleanOr if left.kind == Kind::Bool => Kind::Bool,
                Multiply | Divide | Remainder | Add | Subtract | ShiftLeft | ShiftRight
                | BitAnd | BitOr | BitXor
                    if left.kind == Kind::Word =>
                {
                    Kind::Word
                }
                _ => return Err(Refusal::InvalidProgram),
            };
            if kind != expected {
                return Err(Refusal::InvalidProgram);
            }
            Operation::Binary(*operator, *proven, left, right)
        }
        PreparedOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            let condition = Box::new(lower(condition, nodes)?);
            let yes = Box::new(lower(when_true, nodes)?);
            let no = Box::new(lower(when_false, nodes)?);
            if condition.kind != Kind::Bool || yes.kind != kind || no.kind != kind {
                return Err(Refusal::InvalidProgram);
            }
            Operation::Conditional(condition, yes, no)
        }
        _ => return Err(unsupported()),
    };
    Ok(Node { kind, operation })
}
impl Node {
    fn evaluate(&self, input: u64) -> Result<Value, Refusal> {
        match &self.operation {
            Operation::Input => Ok(Value::Word(input)),
            Operation::Literal(value) => Ok(*value),
            Operation::Unary(UnaryOperator::Not, operand) => {
                Ok(Value::Bool(!operand.evaluate(input)?.boolean()?))
            }
            Operation::Unary(UnaryOperator::Negate, operand) => {
                // The existing fixed-integer contract rejects unsigned
                // negation, including zero; do not introduce a new law here.
                let _ = operand.evaluate(input)?.word()?;
                Err(Refusal::Arithmetic)
            }
            Operation::Binary(operator, proven, left, right) => binary(
                *operator,
                *proven,
                left.evaluate(input)?,
                right.evaluate(input)?,
            ),
            Operation::Conditional(condition, yes, no) => {
                if condition.evaluate(input)?.boolean()? {
                    yes.evaluate(input)
                } else {
                    no.evaluate(input)
                }
            }
        }
    }
}
fn binary(
    operator: BinaryOperator,
    proven: bool,
    left: Value,
    right: Value,
) -> Result<Value, Refusal> {
    use BinaryOperator::*;
    if matches!(
        operator,
        Equal | NotEqual | Less | LessOrEqual | Greater | GreaterOrEqual
    ) {
        let a = left.ordered();
        let b = right.ordered();
        return Ok(Value::Bool(match operator {
            Equal => a == b,
            NotEqual => a != b,
            Less => a < b,
            LessOrEqual => a <= b,
            Greater => a > b,
            GreaterOrEqual => a >= b,
            _ => unreachable!(),
        }));
    }
    if matches!(operator, BooleanAnd | BooleanOr) {
        let a = left.boolean()?;
        let b = right.boolean()?;
        return Ok(Value::Bool(if operator == BooleanAnd {
            a && b
        } else {
            a || b
        }));
    }
    let a = left.word()?;
    let b = right.word()?;
    let value = match operator {
        Add if proven => Some(a.wrapping_add(b)),
        Subtract if proven => Some(a.wrapping_sub(b)),
        Multiply if proven => Some(a.wrapping_mul(b)),
        Add => a.checked_add(b),
        Subtract => a.checked_sub(b),
        Multiply => a.checked_mul(b),
        Divide => a.checked_div(b),
        Remainder => a.checked_rem(b),
        BitAnd => Some(a & b),
        BitOr => Some(a | b),
        BitXor => Some(a ^ b),
        ShiftLeft => u32::try_from(b)
            .ok()
            .filter(|n| *n < 64)
            .and_then(|n| a.checked_mul(1u64 << n)),
        ShiftRight => u32::try_from(b).ok().filter(|n| *n < 64).map(|n| a >> n),
        _ => return Err(Refusal::InvalidProgram),
    }
    .ok_or(Refusal::Arithmetic)?;
    Ok(Value::Word(value))
}
