//! Bounded readable exact expressions. Wire identity does not depend on rendering.
use super::*;
use alloc::{
    format,
    string::{String, ToString},
};
impl ExactRational {
    pub fn limb_count(&self) -> usize {
        self.numerator.len.max(self.denominator.len)
    }
    pub const fn owned_storage_bytes(&self) -> usize {
        core::mem::size_of::<Self>()
    }
    pub fn source_expression(&self) -> String {
        fn decimal(value: &integer::Magnitude) -> String {
            if value.len == 0 {
                return "0".to_string();
            }
            let mut text = value.limbs[value.len - 1].to_string();
            for limb in value.limbs[..value.len - 1].iter().rev() {
                text.push_str(&format!("{limb:09}"));
            }
            text
        }
        let sign = if self.negative { "-" } else { "" };
        let numerator = decimal(&self.numerator);
        if self.denominator.is_one() {
            format!("{sign}{numerator}")
        } else {
            format!("({sign}{numerator} / {})", decimal(&self.denominator))
        }
    }
}
impl MathValue {
    /// Native decimal literal spelling when exact and within the caller's byte
    /// budget. Nonterminating rationals and symbolic values have no such literal.
    pub fn terminating_decimal_literal(
        &self,
        maximum_bytes: usize,
    ) -> Result<Option<String>, NumericRefusal> {
        if maximum_bytes > MAX_NUMERIC_ENCODED_BYTES {
            return Err(NumericRefusal::StorageBudget);
        }
        let Some(value) = self.as_rational() else {
            return Ok(None);
        };
        let mut denominator = value.denominator.clone();
        let mut twos = 0usize;
        let mut fives = 0usize;
        for (factor, count) in [(2u64, &mut twos), (5u64, &mut fives)] {
            while !denominator.is_one() {
                let (quotient, remainder) =
                    denominator.div_rem(&integer::Magnitude::from_u64(factor))?;
                if !remainder.is_zero() {
                    break;
                }
                denominator = quotient;
                *count += 1;
                if *count > maximum_bytes {
                    return Ok(None);
                }
            }
        }
        if !denominator.is_one() {
            return Ok(None);
        }
        let places = twos.max(fives);
        let mut numerator = value.numerator.clone();
        for _ in twos..places {
            numerator = numerator.mul_small(2)?;
        }
        for _ in fives..places {
            numerator = numerator.mul_small(5)?;
        }
        let integer = ExactRational::new(false, numerator, integer::Magnitude::from_u64(1))?
            .source_expression();
        let mut text = if places == 0 {
            integer
        } else if integer.len() > places {
            let at = integer.len() - places;
            format!("{}.{}", &integer[..at], &integer[at..])
        } else {
            let mut out = String::from("0.");
            for _ in integer.len()..places {
                out.push('0');
            }
            out.push_str(&integer);
            out
        };
        if value.negative {
            text.insert(0, '-');
        }
        Ok((text.len() <= maximum_bytes).then_some(text))
    }
    /// Retained Rust storage, including this value, boxed carriers, and actual
    /// vector capacities. Allocator bookkeeping is supplied by the Host budget.
    pub fn owned_storage_bytes(&self) -> usize {
        fn heap(value: &MathValue) -> usize {
            match value {
                MathValue::SmallInteger(_) => 0,
                MathValue::Rational(_) => core::mem::size_of::<ExactRational>(),
                MathValue::Symbolic(graph) => {
                    core::mem::size_of::<SymbolicDag>()
                        + graph.nodes.capacity() * core::mem::size_of::<symbolic::Node>()
                        + graph
                            .nodes
                            .iter()
                            .map(|node| match node {
                                symbolic::Node::Literal(value) => heap(value),
                                symbolic::Node::Operation { operands, .. } => {
                                    operands.capacity() * core::mem::size_of::<u16>()
                                }
                            })
                            .sum::<usize>()
                }
            }
        }
        core::mem::size_of::<Self>() + heap(self)
    }
    /// Exact mathematical expression. Turn-coordinate operators are explicitly
    /// named; a language adapter supplies typed Angle constructors if needed.
    pub fn source_expression(&self) -> Result<String, NumericRefusal> {
        match self {
            Self::SmallInteger(n) => Ok(n.to_string()),
            Self::Rational(n) => Ok(n.source_expression()),
            Self::Symbolic(graph) => {
                let mut rendered = Vec::<String>::new();
                let mut retained = 0usize;
                for node in graph.nodes() {
                    let text = match node {
                        SymbolicNodeRef::Literal(value) => value.source_expression()?,
                        SymbolicNodeRef::Apply {
                            operation,
                            operands,
                        } => {
                            let a = operands
                                .first()
                                .map(|i| rendered[usize::from(*i)].as_str())
                                .unwrap_or("");
                            let b = operands
                                .get(1)
                                .map(|i| rendered[usize::from(*i)].as_str())
                                .unwrap_or("");
                            match operation {
                                SymbolicOperation::Pi => "pi".to_string(),
                                SymbolicOperation::Root(2) => format!("sqrt({a})"),
                                SymbolicOperation::Root(degree) => format!("root({a}, {degree})"),
                                SymbolicOperation::Negate => format!("(-{a})"),
                                SymbolicOperation::Add => format!("({a} + {b})"),
                                SymbolicOperation::Multiply => format!("({a} * {b})"),
                                SymbolicOperation::Divide => format!("({a} / {b})"),
                                SymbolicOperation::Sin => format!("sin_turns({a})"),
                                SymbolicOperation::Cos => format!("cos_turns({a})"),
                                SymbolicOperation::Tan => format!("tan_turns({a})"),
                                SymbolicOperation::Asin => format!("asin_turns({a})"),
                                SymbolicOperation::Acos => format!("acos_turns({a})"),
                                SymbolicOperation::Atan => format!("atan_turns({a})"),
                                SymbolicOperation::Atan2 => format!("atan2_turns({a}, {b})"),
                            }
                        }
                    };
                    retained = retained
                        .checked_add(text.len())
                        .ok_or(NumericRefusal::StorageBudget)?;
                    if retained > MAX_NUMERIC_ENCODED_BYTES {
                        return Err(NumericRefusal::StorageBudget);
                    }
                    rendered.push(text);
                }
                rendered.pop().ok_or(NumericRefusal::MalformedEncoding)
            }
        }
    }
}
