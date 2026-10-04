//! Preparation-only exact schemas for nested expression members.
use super::*;

pub(super) fn registry(
    roots: &BTreeMap<KindId, StructuredInfoType>,
) -> BTreeMap<KindId, StructuredInfoType> {
    let mut result = roots.clone();
    for ty in roots.values() {
        retain(ty, &mut result);
    }
    result
}

fn retain(ty: &StructuredInfoType, result: &mut BTreeMap<KindId, StructuredInfoType>) {
    let kind = ty
        .profile()
        .expect("checked structure has a finite profile")
        .value_kind()
        .clone();
    result.entry(kind).or_insert_with(|| ty.clone());
    match ty.shape() {
        StructuredInfoTypeShape::Leaf(_) => (),
        StructuredInfoTypeShape::Nominal { representation, .. } => retain(representation, result),
        StructuredInfoTypeShape::Record { fields, .. } => {
            for field in fields {
                retain(field.value_type(), result);
            }
        }
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => retain(element, result),
        StructuredInfoTypeShape::Variant { cases, .. } => {
            for case in cases {
                retain(case.payload_type(), result);
            }
        }
    }
}

pub(super) fn member(ty: &StructuredInfoType) -> CheckedExpressionType {
    match ty.shape() {
        StructuredInfoTypeShape::Record { .. } => CheckedExpressionType::Semantic(
            ty.profile()
                .expect("checked record has a finite profile")
                .value_kind()
                .clone(),
        ),
        _ => CheckedExpressionType::from_structured(ty),
    }
}
