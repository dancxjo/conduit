//! Structural protocol recognition; consumption support remains profile-specific.
use crate::{StructuredInfoType, StructuredInfoTypeShape as Shape};

fn type_field<'a>(ty: &'a StructuredInfoType, name: &str) -> Option<&'a StructuredInfoType> {
    let Shape::Record { fields, .. } = ty.shape() else {
        return None;
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .map(|f| f.value_type())
}
fn leaf_type(ty: &StructuredInfoType, id: &str) -> bool {
    matches!(ty.shape(),Shape::Leaf(kind) if kind.as_str()==id)
}
fn empty_cases(ty: &StructuredInfoType, tags: &[&str]) -> bool {
    let Shape::Variant { cases, .. } = ty.shape() else {
        return false;
    };
    cases.len() == tags.len()
        && cases
            .iter()
            .all(|c| tags.contains(&c.tag()) && leaf_type(c.payload_type(), crate::EMPTY_INFO_ID))
}
pub(super) fn protocol_schema(ty: &StructuredInfoType) -> bool {
    let Shape::Record { fields, .. } = ty.shape() else {
        return false;
    };
    if !(9..=10).contains(&fields.len()) {
        return false;
    }
    let report = type_field(ty, "projection-evidence");
    if (fields.len() == 10) != report.is_some()
        || report.is_some_and(|t| !leaf_type(t, crate::BYTES_INFO_ID))
    {
        return false;
    }
    let root_report = report.is_some();
    for (name, id) in [
        ("source", crate::MATHEMATICAL_VALUE_ID),
        ("source-wire", crate::BYTES_INFO_ID),
        ("unit", crate::UNIT_INFO_ID),
        ("requested-limit", crate::QUANTITY_INFO_ID),
        ("limit", crate::MATHEMATICAL_VALUE_ID),
        ("maximum-error", crate::MATHEMATICAL_VALUE_ID),
    ] {
        if !type_field(ty, name).is_some_and(|t| leaf_type(t, id)) {
            return false;
        }
    }
    if !type_field(ty, "role").is_some_and(|t| empty_cases(t, &["linear", "point", "delta"]))
        || !type_field(ty, "claim")
            .is_some_and(|t| empty_cases(t, &["exact_distance", "certified_bound"]))
    {
        return false;
    }
    let Some(rep) = type_field(ty, "representation") else {
        return false;
    };
    let Shape::Variant { cases, .. } = rep.shape() else {
        return false;
    };
    if !(2..=4).contains(&cases.len()) {
        return false;
    }
    for c in cases {
        match c.tag() {
            "exact_rational" if leaf_type(c.payload_type(), crate::MATHEMATICAL_VALUE_ID) => (),
            "binary64_computation" => {
                // Definition recognition is separate from execution support.
                // The ordinary reader still refuses this representation; an
                // explicitly prepared Source consumer must replay its artifacts.
                let Shape::Record { fields, .. } = c.payload_type().shape() else {
                    return false;
                };
                if !root_report
                    || fields.len() != 4
                    || !type_field(c.payload_type(), "target")
                        .is_some_and(|t| leaf_type(t, crate::F64_INFO_ID))
                    || !["program", "incoming"].iter().all(|name| {
                        type_field(c.payload_type(), name)
                            .is_some_and(|t| leaf_type(t, crate::BYTES_INFO_ID))
                    })
                    || !type_field(c.payload_type(), "rounding").is_some_and(|t| {
                        empty_cases(
                            t,
                            &[
                                "nearest_ties_even",
                                "toward_zero",
                                "toward_positive",
                                "toward_negative",
                            ],
                        )
                    })
                {
                    return false;
                }
            }
            "binary32" | "enclosed_binary32" => {
                let Shape::Record { fields, .. } = c.payload_type().shape() else {
                    return false;
                };
                // Historical enclosed payloads had two fields before full
                // report transport, then three with an embedded report.
                let mathematical =
                    c.tag() == "enclosed_binary32" && !root_report && fields.len() == 3;
                if fields.len() != if mathematical { 3 } else { 2 }
                    || (mathematical
                        && !type_field(c.payload_type(), "projection-evidence")
                            .is_some_and(|t| leaf_type(t, crate::BYTES_INFO_ID)))
                    || !type_field(c.payload_type(), "target")
                        .is_some_and(|t| leaf_type(t, crate::F32_INFO_ID))
                    || !type_field(c.payload_type(), "rounding").is_some_and(|t| {
                        empty_cases(
                            t,
                            &[
                                "nearest_ties_even",
                                "toward_zero",
                                "toward_positive",
                                "toward_negative",
                            ],
                        )
                    })
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}
