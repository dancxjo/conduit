//! Capacity-based upper bound for this prepared projection owner. Shared Type
//! nodes may be counted more than once; allocator overhead/stacks are excluded.
use super::PreparedAmbientFaces;
use conduit_plot::{
    CheckedNativeType, PortableExpressionNode as Node, PortableExpressionOperation as Op,
    PortableExpressionProgram as Program, PortableExpressionProjection as Projection,
};
use core::mem::size_of;

impl PreparedAmbientFaces {
    /// Prepared heap payload only, separate from the input, transient projection
    /// workspace and escaped Face. Unknown retained expressions refuse admission.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let validators = self
            .home_validator
            .retained_backing_bytes()?
            .checked_add(self.clock_validator.retained_backing_bytes()?)?
            .checked_add(self.wallpaper_validator.retained_backing_bytes()?)?;
        [
            &self.wallpaper,
            &self.clock_weather,
            &self.home,
            &self.civil,
            &self.wallpaper_observation,
        ]
        .into_iter()
        .try_fold(validators, |n, ty| n.checked_add(native(ty)?))
    }
}
fn vector<T>(values: &alloc::vec::Vec<T>) -> Option<usize> {
    values.capacity().checked_mul(size_of::<T>())
}
fn native(ty: &CheckedNativeType) -> Option<usize> {
    let mut n = ty
        .name
        .capacity()
        .checked_add(ty.identity.0.capacity())?
        .checked_add(ty.value_type.retained_backing_bytes()?)?
        .checked_add(vector(&ty.value_contracts)?)?
        .checked_add(vector(&ty.invariants)?)?;
    for value in &ty.value_contracts {
        n = n
            .checked_add(value.representation_path.capacity())?
            .checked_add(value.contract.retained_backing_bytes()?)?;
    }
    for value in &ty.invariants {
        n = n.checked_add(program(value)?)?;
    }
    Some(n)
}
fn program(p: &Program) -> Option<usize> {
    p.input_type
        .retained_backing_bytes()?
        .checked_add(p.output_type.retained_backing_bytes()?)?
        .checked_add(node(&p.root)?)
}
fn boxed(n: &Node) -> Option<usize> {
    size_of::<Node>().checked_add(node(n)?)
}
fn nodes(values: &alloc::vec::Vec<Node>) -> Option<usize> {
    values
        .iter()
        .try_fold(vector(values)?, |n, value| n.checked_add(node(value)?))
}
fn node(value: &Node) -> Option<usize> {
    let owned = match &value.operation {
        Op::Input => 0,
        Op::Literal(value) => value.capacity(),
        Op::CanonicalLiteral(value) => value.capacity(),
        // The closed ambient law programs do not retain constructor results.
        // Their private storage has no capacity visitor; never use encoded size
        // as a substitute for that missing accounting contract.
        Op::Constant(_) => return None,
        Op::ValueContract {
            input,
            output,
            value,
        } => {
            let mut n = boxed(value)?;
            for contract in [input, output].into_iter().flatten() {
                n = n.checked_add(contract.retained_backing_bytes()?)?;
            }
            n
        }
        Op::Projection { value, member } => boxed(value)?.checked_add(match member {
            Projection::Field(name) => name.capacity(),
            Projection::TupleIndex(_) => 0,
        })?,
        Op::Unary { operand, .. } => boxed(operand)?,
        Op::Binary { left, right, .. } => boxed(left)?.checked_add(boxed(right)?)?,
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => boxed(condition)?
            .checked_add(boxed(when_true)?)?
            .checked_add(boxed(when_false)?)?,
        Op::Tuple(values) | Op::Collection(values) => nodes(values)?,
        Op::Record(fields) => fields
            .iter()
            .try_fold(vector(fields)?, |n, (name, value)| {
                n.checked_add(name.capacity())?.checked_add(node(value)?)
            })?,
        Op::Variant { tag, payload } => tag.capacity().checked_add(boxed(payload)?)?,
        Op::SemanticCall { kind, arguments } => kind.capacity().checked_add(nodes(arguments)?)?,
    };
    value
        .value_type
        .retained_backing_bytes()?
        .checked_add(owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambient_preparation_accounts_for_all_contracts_and_spare_capacity() {
        let prepared = PreparedAmbientFaces::prepare().unwrap();
        assert!(prepared.retained_backing_bytes().unwrap() > 0);
        let mut ty = prepared.wallpaper_type().clone();
        let before = native(&ty).unwrap();
        let capacity = ty.value_contracts.capacity();
        ty.value_contracts.reserve(capacity + 3);
        assert_eq!(
            native(&ty).unwrap() - before,
            (ty.value_contracts.capacity() - capacity)
                * size_of::<conduit_plot::NativeTypeValueContract>()
        );
    }
}
