//! Typed civil labels using the original temporal owner declarations.
use crate::AmbientFaceRefusal;
use alloc::vec::Vec;
use conduit_core::{
    CheckedValueContract, LocalDate, LocalTime, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_face::FacePropertyValue;
use conduit_plot::rust_binding::semantic_core as conduit_core;
use conduit_plot::{
    check_syntax_document, parse_syntax_document, rust_binding, CheckedNativeType, StartupCatalog,
};

pub(crate) fn prepare() -> Result<CheckedNativeType, AmbientFaceRefusal> {
    let mut source =
        parse_syntax_document(include_str!("../../../../semantics/time/types.conduit"));
    source.types.retain(|ty| {
        matches!(
            ty.name.text.as_str(),
            "LocalDate" | "LocalTime" | "LocalDateTime"
        )
    });
    source.type_forms.clear();
    let checked = check_syntax_document(&source, &StartupCatalog::new())
        .map_err(|_| AmbientFaceRefusal::TypePreparation)?;
    checked
        .native_types
        .into_iter()
        .find(|ty| ty.name == "LocalDateTime")
        .ok_or(AmbientFaceRefusal::TypePreparation)
}

pub(crate) fn property(
    ty: &CheckedNativeType,
    date: &str,
    time: &str,
) -> Result<FacePropertyValue, AmbientFaceRefusal> {
    let invalid = || AmbientFaceRefusal::InvalidObservation;
    // The supplied grammar is fixed ASCII. Calendar validity comes from the
    // existing civil primitives; no timezone or clock is inferred here.
    let d = date.as_bytes();
    let t = time.as_bytes();
    if d.len() != 10
        || t.len() != 8
        || d[4] != b'-'
        || d[7] != b'-'
        || t[2] != b':'
        || t[5] != b':'
        || !d
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
        || !t
            .iter()
            .enumerate()
            .all(|(i, b)| i == 2 || i == 5 || b.is_ascii_digit())
    {
        return Err(invalid());
    }
    let date = LocalDate::new(
        date[..4].parse().map_err(|_| invalid())?,
        date[5..7].parse().map_err(|_| invalid())?,
        date[8..].parse().map_err(|_| invalid())?,
    )
    .map_err(|_| invalid())?;
    let time = LocalTime::new(
        time[..2].parse().map_err(|_| invalid())?,
        time[3..5].parse().map_err(|_| invalid())?,
        time[6..].parse().map_err(|_| invalid())?,
        0,
    )
    .map_err(|_| invalid())?;
    let value = record(&ty.value_type, |name, ty| match name {
        "date" => record(ty, |name, ty| match name {
            "year" => primitive(ty, &date.year()),
            "month" => primitive(ty, &date.month()),
            "day" => primitive(ty, &date.day()),
            _ => Err(invalid()),
        }),
        "time" => record(ty, |name, ty| match name {
            "hour" => primitive(ty, &time.hour()),
            "minute" => primitive(ty, &time.minute()),
            "second" => primitive(ty, &time.second()),
            "nanosecond" => primitive(ty, &time.nanosecond()),
            _ => Err(invalid()),
        }),
        _ => Err(invalid()),
    })?;
    let bytes = value.canonical_bytes().map_err(|_| invalid())?;
    let profile = value.value_type().profile().map_err(|_| invalid())?;
    let contract = CheckedValueContract::new(profile.value_kind().clone(), 1024, Vec::new())
        .map_err(|_| invalid())?;
    contract.validate(&bytes).map_err(|_| invalid())?;
    Ok(FacePropertyValue::TypedValue { contract, bytes })
}

fn primitive<T: rust_binding::NativePrimitive>(
    ty: &StructuredInfoType,
    value: &T,
) -> Result<StructuredInfoValue, AmbientFaceRefusal> {
    if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
        return StructuredInfoValue::nominal(ty.clone(), primitive(representation, value)?)
            .map_err(|_| AmbientFaceRefusal::InvalidObservation);
    }
    rust_binding::primitive_into_structured(ty.clone(), value)
        .map_err(|_| AmbientFaceRefusal::InvalidObservation)
}

fn record(
    ty: &StructuredInfoType,
    mut value: impl FnMut(&str, &StructuredInfoType) -> Result<StructuredInfoValue, AmbientFaceRefusal>,
) -> Result<StructuredInfoValue, AmbientFaceRefusal> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(AmbientFaceRefusal::InvalidObservation);
    };
    let fields = fields
        .iter()
        .map(|field| {
            StructuredFieldValue::new(field.name(), value(field.name(), field.value_type())?)
                .map_err(|_| AmbientFaceRefusal::InvalidObservation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    StructuredInfoValue::record(ty.clone(), fields)
        .map_err(|_| AmbientFaceRefusal::InvalidObservation)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_validity_is_checked_before_typed_presentation() {
        let ty = prepare().unwrap();
        assert!(property(&ty, "2024-02-29", "23:59:59").is_ok());
        for (date, time) in [
            ("2025-02-29", "12:00:00"),
            ("2026-04-31", "12:00:00"),
            ("2026-11-15", "24:00:00"),
            ("2026-11-15", "12:00:60"),
            ("é026-11-15", "12:00:00"),
        ] {
            assert_eq!(
                property(&ty, date, time),
                Err(AmbientFaceRefusal::InvalidObservation),
                "{date} {time}"
            );
        }
    }
}
