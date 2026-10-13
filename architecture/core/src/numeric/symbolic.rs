use super::*;
use alloc::vec;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolicOperation {
    Pi,
    Root(u8),
    Negate,
    Add,
    Multiply,
    Divide,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
}
impl SymbolicOperation {
    pub(super) fn code(self) -> (u8, u8) {
        match self {
            Self::Pi => (1, 0),
            Self::Root(n) => (2, n),
            Self::Negate => (3, 0),
            Self::Add => (4, 0),
            Self::Multiply => (5, 0),
            Self::Divide => (6, 0),
            Self::Sin => (7, 0),
            Self::Cos => (8, 0),
            Self::Tan => (9, 0),
            Self::Asin => (10, 0),
            Self::Acos => (11, 0),
            Self::Atan => (12, 0),
            Self::Atan2 => (13, 0),
        }
    }
    pub(super) fn from_code(code: u8, degree: u8) -> Result<Self, NumericRefusal> {
        Ok(match (code, degree) {
            (1, 0) => Self::Pi,
            (2, 2..=32) => Self::Root(degree),
            (3, 0) => Self::Negate,
            (4, 0) => Self::Add,
            (5, 0) => Self::Multiply,
            (6, 0) => Self::Divide,
            (7, 0) => Self::Sin,
            (8, 0) => Self::Cos,
            (9, 0) => Self::Tan,
            (10, 0) => Self::Asin,
            (11, 0) => Self::Acos,
            (12, 0) => Self::Atan,
            (13, 0) => Self::Atan2,
            _ => return Err(NumericRefusal::NonCanonical),
        })
    }
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Pi => 0,
            Self::Root(_)
            | Self::Negate
            | Self::Sin
            | Self::Cos
            | Self::Tan
            | Self::Asin
            | Self::Acos
            | Self::Atan => 1,
            _ => 2,
        }
    }
    pub(super) fn commutative(self) -> bool {
        matches!(self, Self::Add | Self::Multiply)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Node {
    Literal(MathValue),
    Operation {
        op: SymbolicOperation,
        operands: Vec<u16>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicDag {
    pub(super) nodes: Vec<Node>,
}
#[derive(Debug, Clone, Copy)]
pub enum SymbolicNodeRef<'a> {
    Literal(&'a MathValue),
    Apply {
        operation: SymbolicOperation,
        operands: &'a [u16],
    },
}
impl SymbolicDag {
    pub fn node(&self, index: u16) -> Option<SymbolicNodeRef<'_>> {
        self.nodes.get(usize::from(index)).map(|node| match node {
            Node::Literal(value) => SymbolicNodeRef::Literal(value),
            Node::Operation { op, operands } => SymbolicNodeRef::Apply {
                operation: *op,
                operands,
            },
        })
    }
    pub fn nodes(&self) -> impl ExactSizeIterator<Item = SymbolicNodeRef<'_>> {
        self.nodes.iter().map(|node| match node {
            Node::Literal(value) => SymbolicNodeRef::Literal(value),
            Node::Operation { op, operands } => SymbolicNodeRef::Apply {
                operation: *op,
                operands,
            },
        })
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn root_operation(&self) -> SymbolicOperation {
        match self.nodes.last().expect("admitted nonempty DAG") {
            Node::Operation { op, .. } => *op,
            Node::Literal(_) => unreachable!("symbolic root"),
        }
    }
}
fn push(nodes: &mut Vec<Node>, node: Node) -> Result<u16, NumericRefusal> {
    if let Some(i) = nodes.iter().position(|n| n == &node) {
        return Ok(i as u16);
    }
    if nodes.len() >= MAX_SYMBOLIC_NODES {
        return Err(NumericRefusal::NodeBudget);
    }
    nodes.push(node);
    Ok((nodes.len() - 1) as u16)
}
fn import(nodes: &mut Vec<Node>, value: &MathValue) -> Result<u16, NumericRefusal> {
    match value {
        MathValue::Symbolic(dag) => {
            let mut mapping = [0u16; MAX_SYMBOLIC_NODES];
            for (i, node) in dag.nodes.iter().enumerate() {
                let mapped = match node {
                    Node::Literal(v) => Node::Literal(v.clone()),
                    Node::Operation { op, operands } => Node::Operation {
                        op: *op,
                        operands: operands.iter().map(|i| mapping[usize::from(*i)]).collect(),
                    },
                };
                mapping[i] = push(nodes, mapped)?;
            }
            Ok(mapping[dag.nodes.len() - 1])
        }
        _ => push(nodes, Node::Literal(value.clone())),
    }
}
fn expression(op: SymbolicOperation, values: &[&MathValue]) -> Result<MathValue, NumericRefusal> {
    let mut nodes = Vec::new();
    let mut ordered = values.to_vec();
    if op.commutative() && super::codec::key(ordered[0])? > super::codec::key(ordered[1])? {
        ordered.swap(0, 1);
    }
    let mut operands = Vec::new();
    for value in ordered {
        operands.push(import(&mut nodes, value)?);
    }
    push(&mut nodes, Node::Operation { op, operands })?;
    let value = MathValue::Symbolic(Box::new(SymbolicDag { nodes }));
    value.canonical_bytes()?;
    Ok(value)
}
pub(super) fn pi() -> MathValue {
    MathValue::Symbolic(Box::new(SymbolicDag {
        nodes: vec![Node::Operation {
            op: SymbolicOperation::Pi,
            operands: Vec::new(),
        }],
    }))
}
pub(super) fn unary(op: SymbolicOperation, value: &MathValue) -> Result<MathValue, NumericRefusal> {
    if op == SymbolicOperation::Negate {
        if let Some(coefficient) = pi_factor(value)? {
            return pi_multiple(coefficient.negated());
        }
    }
    if op == SymbolicOperation::Negate {
        if let MathValue::Symbolic(dag) = value {
            if let Some(Node::Operation {
                op: SymbolicOperation::Negate,
                operands,
            }) = dag.nodes.last()
            {
                return extract(dag, operands[0]);
            }
        }
    }
    expression(op, &[value])
}
fn extract(dag: &SymbolicDag, index: u16) -> Result<MathValue, NumericRefusal> {
    let root = usize::from(index);
    let Some(node) = dag.nodes.get(root) else {
        return Err(NumericRefusal::MalformedEncoding);
    };
    if let Node::Literal(value) = node {
        return Ok(value.clone());
    }
    let mut reachable = [false; MAX_SYMBOLIC_NODES];
    reachable[root] = true;
    for index in (0..=root).rev() {
        if reachable[index] {
            if let Node::Operation { operands, .. } = &dag.nodes[index] {
                for operand in operands {
                    reachable[usize::from(*operand)] = true;
                }
            }
        }
    }
    let count = reachable[..=root].iter().filter(|node| **node).count();
    let mut nodes = Vec::with_capacity(count);
    let mut mapping = [0; MAX_SYMBOLIC_NODES];
    for (index, node) in dag.nodes[..=root].iter().enumerate() {
        if !reachable[index] {
            continue;
        }
        let node = match node {
            Node::Literal(value) => Node::Literal(value.clone()),
            Node::Operation { op, operands } => Node::Operation {
                op: *op,
                operands: operands
                    .iter()
                    .map(|operand| mapping[usize::from(*operand)])
                    .collect(),
            },
        };
        mapping[index] = u16::try_from(nodes.len()).map_err(|_| NumericRefusal::NodeBudget)?;
        nodes.push(node);
    }
    Ok(MathValue::Symbolic(Box::new(SymbolicDag { nodes })))
}
pub(super) fn binary(
    op: SymbolicOperation,
    left: &MathValue,
    right: &MathValue,
) -> Result<MathValue, NumericRefusal> {
    if op == SymbolicOperation::Add {
        for (value, negative) in [(left, right), (right, left)] {
            if let MathValue::Symbolic(dag) = negative {
                if let Some(Node::Operation {
                    op: SymbolicOperation::Negate,
                    operands,
                }) = dag.nodes.last()
                {
                    if &extract(dag, operands[0])? == value {
                        return Ok(MathValue::ZERO);
                    }
                }
            }
        }
    }
    if op == SymbolicOperation::Multiply {
        for (quotient, factor) in [(left, right), (right, left)] {
            if let MathValue::Symbolic(dag) = quotient {
                if let Some(Node::Operation {
                    op: SymbolicOperation::Divide,
                    operands,
                }) = dag.nodes.last()
                {
                    let denominator = extract(dag, operands[1])?;
                    if &denominator == factor && denominator.proven_nonzero() {
                        return extract(dag, operands[0]);
                    }
                }
            }
        }
        if left == right {
            if let MathValue::Symbolic(dag) = left {
                if let Some(Node::Operation {
                    op: SymbolicOperation::Root(2),
                    operands,
                }) = dag.nodes.last()
                {
                    return extract(dag, operands[0]);
                }
            }
        }
    }
    match op {
        SymbolicOperation::Add => {
            if left == &MathValue::ZERO {
                return Ok(right.clone());
            }
            if right == &MathValue::ZERO {
                return Ok(left.clone());
            }
        }
        SymbolicOperation::Multiply => {
            if left == &MathValue::ZERO || right == &MathValue::ZERO {
                return Ok(MathValue::ZERO);
            }
            if left == &MathValue::ONE {
                return Ok(right.clone());
            }
            if right == &MathValue::ONE {
                return Ok(left.clone());
            }
        }
        SymbolicOperation::Divide => {
            if left == &MathValue::ZERO {
                return Ok(MathValue::ZERO);
            }
            if right == &MathValue::ONE {
                return Ok(left.clone());
            }
            if left == right && right.proven_nonzero() {
                return Ok(MathValue::ONE);
            }
        }
        _ => {}
    }
    let left_inverse = reciprocal_pi_factor(left)?;
    let right_inverse = reciprocal_pi_factor(right)?;
    if op == SymbolicOperation::Add {
        if let (Some(a), Some(b)) = (&left_inverse, &right_inverse) {
            return reciprocal_pi_multiple(a.checked_add(b)?);
        }
    }
    if op == SymbolicOperation::Multiply {
        if let (Some(a), Some(b)) = (pi_factor(left)?, right_inverse.as_ref()) {
            return Ok(MathValue::from_rational(a.checked_mul(b)?));
        }
        if let (Some(a), Some(b)) = (left_inverse.as_ref(), pi_factor(right)?) {
            return Ok(MathValue::from_rational(a.checked_mul(&b)?));
        }
        if let (Some(a), Some(b)) = (left.as_rational(), right_inverse.as_ref()) {
            return reciprocal_pi_multiple(a.checked_mul(b)?);
        }
        if let (Some(a), Some(b)) = (left_inverse.as_ref(), right.as_rational()) {
            return reciprocal_pi_multiple(a.checked_mul(&b)?);
        }
    }
    if op == SymbolicOperation::Divide {
        if let (Some(a), Some(b)) = (left.as_rational(), pi_factor(right)?) {
            return reciprocal_pi_multiple(a.checked_div(&b)?);
        }
        if let (Some(a), Some(b)) = (left.as_rational(), right_inverse.as_ref()) {
            return pi_multiple(a.checked_div(b)?);
        }
        if let (Some(a), Some(b)) = (left_inverse.as_ref(), right.as_rational()) {
            return reciprocal_pi_multiple(a.checked_div(&b)?);
        }
        if let (Some(a), Some(b)) = (&left_inverse, &right_inverse) {
            return Ok(MathValue::from_rational(a.checked_div(b)?));
        }
    }
    if let (Some(a), Some(b)) = (pi_factor(left)?, pi_factor(right)?) {
        if op == SymbolicOperation::Add {
            return pi_multiple(a.checked_add(&b)?);
        }
        if op == SymbolicOperation::Divide {
            return Ok(MathValue::from_rational(a.checked_div(&b)?));
        }
    }
    if op == SymbolicOperation::Multiply {
        if let (Some(a), Some(b)) = (pi_factor(left)?, right.as_rational()) {
            return pi_multiple(a.checked_mul(&b)?);
        }
        if let (Some(a), Some(b)) = (left.as_rational(), pi_factor(right)?) {
            return pi_multiple(a.checked_mul(&b)?);
        }
    }
    if op == SymbolicOperation::Divide {
        if let (Some(a), Some(b)) = (pi_factor(left)?, right.as_rational()) {
            return pi_multiple(a.checked_div(&b)?);
        }
    }
    expression(op, &[left, right])
}
fn pi_multiple(coefficient: ExactRational) -> Result<MathValue, NumericRefusal> {
    if coefficient.is_zero() {
        return Ok(MathValue::ZERO);
    }
    if coefficient.as_i64() == Some(1) {
        return Ok(pi());
    }
    expression(
        SymbolicOperation::Multiply,
        &[&MathValue::from_rational(coefficient), &pi()],
    )
}
fn reciprocal_pi_multiple(coefficient: ExactRational) -> Result<MathValue, NumericRefusal> {
    if coefficient.is_zero() {
        return Ok(MathValue::ZERO);
    }
    expression(
        SymbolicOperation::Divide,
        &[&MathValue::from_rational(coefficient), &pi()],
    )
}
fn reciprocal_pi_factor(value: &MathValue) -> Result<Option<ExactRational>, NumericRefusal> {
    let MathValue::Symbolic(dag) = value else {
        return Ok(None);
    };
    if let Some(Node::Operation {
        op: SymbolicOperation::Divide,
        operands,
    }) = dag.nodes.last()
    {
        let numerator = extract(dag, operands[0])?;
        let denominator = extract(dag, operands[1])?;
        if let (Some(n), Some(d)) = (numerator.as_rational(), pi_factor(&denominator)?) {
            return Ok(Some(n.checked_div(&d)?));
        }
    }
    Ok(None)
}
fn pi_factor(value: &MathValue) -> Result<Option<ExactRational>, NumericRefusal> {
    let MathValue::Symbolic(dag) = value else {
        return Ok(None);
    };
    match dag.nodes.last().unwrap() {
        Node::Operation {
            op: SymbolicOperation::Pi,
            ..
        } => Ok(Some(ExactRational::from_i64(1))),
        Node::Operation {
            op: SymbolicOperation::Multiply,
            operands,
        } => {
            let a = extract(dag, operands[0])?;
            let b = extract(dag, operands[1])?;
            if a == pi() {
                Ok(b.as_rational())
            } else if b == pi() {
                Ok(a.as_rational())
            } else {
                Ok(None)
            }
        }
        _ => Ok(None),
    }
}
fn sign_facts(value: &MathValue) -> (bool, bool) {
    if let Some(n) = value.as_rational() {
        return (!n.is_negative() && !n.is_zero(), !n.is_zero());
    }
    let MathValue::Symbolic(dag) = value else {
        return (false, false);
    };
    let mut positive = [false; MAX_SYMBOLIC_NODES];
    let mut nonzero = [false; MAX_SYMBOLIC_NODES];
    for (i, node) in dag.nodes.iter().enumerate() {
        let facts = match node {
            Node::Literal(v) => sign_facts(v),
            Node::Operation { op, operands } => {
                let a = operands.first().map(|i| usize::from(*i));
                let b = operands.get(1).map(|i| usize::from(*i));
                match op {
                    SymbolicOperation::Pi => (true, true),
                    SymbolicOperation::Root(_) => (positive[a.unwrap()], nonzero[a.unwrap()]),
                    SymbolicOperation::Negate => (false, nonzero[a.unwrap()]),
                    SymbolicOperation::Multiply | SymbolicOperation::Divide => (
                        positive[a.unwrap()] && positive[b.unwrap()],
                        nonzero[a.unwrap()] && nonzero[b.unwrap()],
                    ),
                    SymbolicOperation::Add => {
                        let p = positive[a.unwrap()] && positive[b.unwrap()];
                        (p, p)
                    }
                    _ => (false, false),
                }
            }
        };
        positive[i] = facts.0;
        nonzero[i] = facts.1;
    }
    (positive[dag.nodes.len() - 1], nonzero[dag.nodes.len() - 1])
}
pub(super) fn nonzero(value: &MathValue) -> bool {
    sign_facts(value).1
}
pub(super) fn positive(value: &MathValue) -> bool {
    sign_facts(value).0
}
pub(super) fn root(value: &MathValue, degree: u8) -> Result<MathValue, NumericRefusal> {
    if !(2..=32).contains(&degree) {
        return Err(NumericRefusal::Domain);
    }
    if let Some(n) = value.as_rational() {
        if n.is_negative() && degree.is_multiple_of(2) {
            return Err(NumericRefusal::Domain);
        }
        if let Some(integer) = n.as_i64() {
            if integer == 0 || integer == 1 {
                return Ok(value.clone());
            }
            let magnitude = integer.unsigned_abs();
            let mut low = 0u64;
            let mut high = magnitude.min(3_037_000_500);
            while low < high {
                let mid = low + (high - low).div_ceil(2);
                if mid
                    .checked_pow(u32::from(degree))
                    .is_some_and(|p| p <= magnitude)
                {
                    low = mid;
                } else {
                    high = mid - 1;
                }
            }
            if low.checked_pow(u32::from(degree)) == Some(magnitude) {
                return Ok(MathValue::integer(if integer < 0 {
                    -(low as i64)
                } else {
                    low as i64
                }));
            }
        }
    } else if !positive(value) {
        return Err(NumericRefusal::IndeterminateDomain);
    }
    expression(SymbolicOperation::Root(degree), &[value])
}
pub(super) fn trig_turns(
    op: SymbolicOperation,
    value: &MathValue,
) -> Result<MathValue, NumericRefusal> {
    if let Some(n) = value.as_rational() {
        let quarter = n.checked_mul(&ExactRational::from_i64(4))?;
        if let Some(q) = quarter.as_i64() {
            let index = q.rem_euclid(4);
            return match op {
                SymbolicOperation::Sin => Ok(MathValue::integer([0, 1, 0, -1][index as usize])),
                SymbolicOperation::Cos => Ok(MathValue::integer([1, 0, -1, 0][index as usize])),
                SymbolicOperation::Tan => {
                    if index % 2 == 1 {
                        Err(NumericRefusal::Domain)
                    } else {
                        Ok(MathValue::ZERO)
                    }
                }
                _ => unreachable!(),
            };
        }
    } else if op == SymbolicOperation::Tan {
        return Err(NumericRefusal::IndeterminateDomain);
    }
    expression(op, &[value])
}
pub(super) fn inverse(
    op: SymbolicOperation,
    value: &MathValue,
) -> Result<MathValue, NumericRefusal> {
    if matches!(op, SymbolicOperation::Asin | SymbolicOperation::Acos) {
        let Some(n) = value.as_rational() else {
            return Err(NumericRefusal::IndeterminateDomain);
        };
        if n.compare(&ExactRational::from_i64(-1))? == Ordering::Less
            || n.compare(&ExactRational::from_i64(1))? == Ordering::Greater
        {
            return Err(NumericRefusal::Domain);
        }
    }
    if let Some(n) = value.exact_i64() {
        let ratio = match (op, n) {
            (SymbolicOperation::Asin, 0) | (SymbolicOperation::Atan, 0) => Some((0, 1)),
            (SymbolicOperation::Asin, 1) => Some((1, 4)),
            (SymbolicOperation::Asin, -1) => Some((-1, 4)),
            (SymbolicOperation::Acos, 1) => Some((0, 1)),
            (SymbolicOperation::Acos, 0) => Some((1, 4)),
            (SymbolicOperation::Acos, -1) => Some((1, 2)),
            (SymbolicOperation::Atan, 1) => Some((1, 8)),
            (SymbolicOperation::Atan, -1) => Some((-1, 8)),
            _ => None,
        };
        if let Some((n, d)) = ratio {
            return MathValue::rational(&alloc::format!("{n}"), &alloc::format!("{d}"));
        }
    }
    expression(op, &[value])
}
pub(super) fn atan2(y: &MathValue, x: &MathValue) -> Result<MathValue, NumericRefusal> {
    if let (Some(y), Some(x)) = (y.as_rational(), x.as_rational()) {
        if x.is_zero() && y.is_zero() {
            return Err(NumericRefusal::Domain);
        }
        if y.is_zero() {
            return if x.is_negative() {
                MathValue::rational("1", "2")
            } else {
                Ok(MathValue::ZERO)
            };
        }
        if x.is_zero() {
            return MathValue::rational(if y.is_negative() { "-1" } else { "1" }, "4");
        }
    } else if !x.proven_nonzero() && !y.proven_nonzero() {
        return Err(NumericRefusal::IndeterminateDomain);
    }
    expression(SymbolicOperation::Atan2, &[y, x])
}
