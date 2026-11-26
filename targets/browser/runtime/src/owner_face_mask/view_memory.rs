//! Requested storage for the complete generic (component=None) FaceView.
//! The selected executor must reject a configured component before calling this
//! model and fence every copied non-Face root/route identifier at 256 bytes.
//! Source Face, Plan, codec buffers and Face-validation workspace are separate
//! reservations. This counts preparation temporaries as well as retained output;
//! it is not an allocator-confinement or physical allocator-overhead guarantee.
use super::*;
use conduit_face::{FacePropertyValue as V, FaceRole};
use core::{fmt, mem::size_of};

const ID: usize = conduit_face::MAX_FACE_ID_BYTES;
const TEXT: usize = conduit_face::MAX_FACE_TEXT_BYTES;
const UNIT: usize = conduit_core::UNIT_MAX_SOURCE_BYTES;
const U64: usize = 20;
// display_typed_value decimal: coefficient <=38 digits, exponent <=128,
// optional leading zero/dot/sign. Canonical Unit spelling is independently128B.
const DECIMAL: usize = 38 + 128 + 2;
const TYPED_TEXT: usize = DECIMAL + 1 + UNIT;
// Plot labels use i128 and i16; decimal branch adds at most six places.
const PLOT_NUMBER: usize = 40 + 1 + 6;
const PLOT_LABEL: usize = 40 + 1 + 6 + 1 + UNIT;

pub(super) fn view_preparation_bytes(face: &Face) -> Result<usize, String> {
    let mut b = Budget(size_of::<FaceView>());
    // Eight root IDs and five optional route IDs, plus decimal revision.
    b.add(13 * ID + U64)?;
    b.table::<SubjectView>(face.subjects.len())?;
    b.table::<RelationshipView>(face.relationships.len())?;
    b.table::<ActionView>(face.actions.len())?;
    let mut series = 0usize;
    let maximum_current_value = face
        .properties
        .iter()
        .filter_map(|p| match &p.value {
            V::TypedValue { bytes, .. } => Some(bytes.len()),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    for subject in &face.subjects {
        b.add(subject.identity.len() + subject.name.len())?;
        b.formatted(debug_len(&subject.role)?)?;
        if let FaceRole::Semantic(role) = &subject.role {
            b.add(role.as_str().len())?;
            if role.as_str() == conduit_face::QUANTITY_SERIES_ROLE {
                series += 1;
            }
        }
        if let Some(disclosure) = face
            .disclosures
            .iter()
            .find(|d| d.subject == subject.identity)
        {
            b.formatted(debug_len(&disclosure.level)?)?;
        } else {
            b.add("Primary".len())?;
        }
        if let Some(text) = face.properties.iter().find_map(|p| {
            if p.subject == subject.identity && p.name == "choice-multiplicity" {
                if let V::Text(value) = &p.value {
                    return Some(value);
                }
            }
            None
        }) {
            b.add(text.len())?;
        }
        let mut properties = 0usize;
        let mut values = 0usize;
        let mut flags = 0usize;
        let mut text_items = 0usize;
        for text in face.text.iter().filter(|t| t.subject == subject.identity) {
            text_items += 1;
            b.add(text.text.len())?;
        }
        for property in face
            .properties
            .iter()
            .filter(|p| p.subject == subject.identity)
        {
            properties += 1;
            let length = property_value_bytes(&property.value, &mut b)?;
            // property_value's temporary String coexists with this complete fact.
            b.formatted(
                property
                    .name
                    .len()
                    .checked_add(2)
                    .and_then(|n| n.checked_add(length))
                    .ok_or_else(overflow)?,
            )?;
            match &property.value {
                V::TypedValue { contract, .. } => {
                    values += 1;
                    b.add(property.name.len() + contract.value_kind.as_str().len())?;
                    typed_wording(&mut b)?;
                }
                V::Flag(_) => {
                    flags += 1;
                    b.add(property.name.len())?;
                }
                _ => {}
            }
        }
        b.table::<String>(properties)?;
        b.table::<String>(text_items)?;
        b.table::<ValueView>(values)?;
        b.table::<FlagView>(flags)?;
    }
    for relationship in &face.relationships {
        b.add(relationship.source.len() + relationship.target.len())?;
        b.formatted(debug_len(&relationship.kind)?)?;
    }
    for action in &face.actions {
        b.add(
            action.identity.len() + action.name.len() + action.intent.len() + action.target.len(),
        )?;
        b.formatted(debug_len(&action.disclosure)?)?;
        match &action.availability {
            conduit_face::FaceActionAvailability::Available => b.add("available".len())?,
            conduit_face::FaceActionAvailability::Unavailable {
                explanation,
                reason_code,
            } => b.add("unavailable".len() + explanation.len() + reason_code.len())?,
            conduit_face::FaceActionAvailability::Refused {
                explanation,
                reason_code,
            } => b.add("refused".len() + explanation.len() + reason_code.len())?,
        }
        b.table::<ArgumentView>(action.arguments.len())?;
        for argument in &action.arguments {
            simple_contract(&argument.contract)?;
            b.add(
                argument.name.len()
                    + argument.value_name.len()
                    + argument.contract.value_kind.as_str().len(),
            )?;
            // current_text_value returns None unless exactly one argument. Its
            // sole possible cloned UTF-8 input is one existing TypedValue.
            if action.arguments.len() == 1 {
                b.add(maximum_current_value)?;
            }
            if let Some(members) = argument.contract.constraints.iter().find_map(|c| match c {
                ValueConstraint::CanonicalMembership {
                    members,
                    negated: false,
                } => Some(members),
                _ => None,
            }) {
                b.table::<String>(members.len())?;
                // Invalid UTF-8 members are dropped, but their temporary clone
                // is still charged rather than treating absence as free work.
                for member in members {
                    b.add(member.len())?;
                }
            }
        }
    }
    if series != 0 {
        quantity_series_storage(&mut b, series.min(8))?;
    }
    Ok(b.0)
}

fn simple_contract(contract: &conduit_core::CheckedValueContract) -> Result<(), String> {
    if contract.constraints.iter().any(|c| {
        !matches!(
            c,
            ValueConstraint::ByteLength { .. } | ValueConstraint::CanonicalMembership { .. }
        )
    }) {
        return Err(
            "compressed generic view does not admit this value constraint workspace".into(),
        );
    }
    Ok(())
}
fn property_value_bytes(value: &V, b: &mut Budget) -> Result<usize, String> {
    let length = match value {
        V::Identity(value) | V::Text(value) => {
            b.add(value.len())?;
            value.len()
        }
        V::BaseImplementationId(value) => {
            b.add(value.as_str().len())?;
            value.as_str().len()
        }
        V::Count(_) => {
            b.add(U64)?;
            U64
        }
        V::Signed(_) => {
            b.add(20)?;
            20
        }
        V::Flag(_) => {
            b.add(5)?;
            5
        }
        V::TypedValue { contract, bytes } => {
            typed_representation(contract, bytes)?;
            if !contract.constraints.is_empty() {
                return Err(
                    "compressed generic typed wording requires an unconstrained value contract"
                        .into(),
                );
            }
            typed_wording(b)?;
            TYPED_TEXT.max(contract.value_kind.as_str().len() + 2 + U64 + " canonical bytes".len())
        }
        V::ValueContract(contract) => {
            simple_contract(contract)?;
            let length = debug_len(contract)?;
            b.formatted(length)?;
            length
        }
        V::Content(_) => {
            let length = U64 + " content bytes".len();
            b.formatted(length)?;
            length
        }
    };
    Ok(length)
}
fn typed_representation(
    contract: &conduit_core::CheckedValueContract,
    bytes: &[u8],
) -> Result<(), String> {
    use conduit_core::PrimitiveInfoKind as P;
    let kind = conduit_core::primitive_info_kind(contract.value_kind.as_str());
    if matches!(
        kind,
        Some(
            P::Quantity
                | P::Distance
                | P::Frequency
                | P::Duration
                | P::Voltage
                | P::Temperature
                | P::Angle
                | P::Ratio
                | P::PixelCount
        )
    ) && conduit_core::Quantity::decode(bytes).is_err()
    {
        return Err(
            "compressed generic wording requires the decimal Quantity representation".into(),
        );
    }
    Ok(())
}
fn typed_wording(b: &mut Budget) -> Result<(), String> {
    // Distinct decimal owners: digits under extension/insertion, leading-zero
    // repeat, replacement decimal String, Unit String and final display String.
    b.formatted(DECIMAL)?;
    b.add(128)?;
    b.formatted(DECIMAL)?;
    b.add(UNIT)?;
    b.formatted(TYPED_TEXT)?;
    Ok(())
}
fn quantity_series_storage(b: &mut Budget, retained: usize) -> Result<(), String> {
    use conduit_face::{ProjectedQuantitySeries, QuantitySeriesPoint, QuantitySeriesSample};
    const SAMPLES: usize = 64;
    const POINTS: usize = 16;
    // prepare/project transients coexist with previously retained projections.
    // These explicit-capacity Vecs never use collect growth.
    b.add(size_of::<conduit_face::PreparedQuantitySeries<'_>>())?;
    b.add(SAMPLES * size_of::<QuantitySeriesSample<'_>>())?;
    b.add(SAMPLES * size_of::<Option<(i128, i16)>>())?;
    b.add(SAMPLES * size_of::<Option<i128>>())?;
    b.add(SAMPLES * size_of::<usize>())?;
    // The retained ProjectedQuantitySeries inline record already lives inside
    // SubjectView's table. Count one additional result under preparation moves.
    b.add(size_of::<ProjectedQuantitySeries>())?;
    for _ in 0..retained {
        b.add(POINTS * size_of::<QuantitySeriesPoint>())?;
        b.add(
            ID + 4 * ID
                + 2 * U64
                + UNIT
                + U64
                + TEXT
                + conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES,
        )?;
        // Minimum/maximum labels each own number-temporary and final String.
        for _ in 0..2 {
            plot_label(b)?;
        }
        for _ in 0..POINTS {
            b.add(4 * U64 + TEXT)?; // sequence/time, optional gap pair, reason
            plot_label(b)?;
        }
    }
    Ok(())
}
fn plot_label(b: &mut Budget) -> Result<(), String> {
    b.formatted(PLOT_NUMBER)?;
    b.formatted(PLOT_LABEL)
}

struct Budget(usize);
impl Budget {
    fn add(&mut self, bytes: usize) -> Result<(), String> {
        self.0 = self.0.checked_add(bytes).ok_or_else(overflow)?;
        Ok(())
    }
    fn formatted(&mut self, length: usize) -> Result<(), String> {
        // String push growth doubles requested capacity;8 is the minimum first
        // allocation. Literal estimates for these format sites fit this bound.
        self.add(
            length
                .checked_mul(2)
                .and_then(|n| n.checked_add(8))
                .ok_or_else(overflow)?,
        )
    }
    fn table<T>(&mut self, count: usize) -> Result<(), String> {
        if count != 0 {
            self.add(
                count
                    .checked_mul(2)
                    .map(|n| n.max(4))
                    .and_then(|n| n.checked_mul(size_of::<T>()))
                    .ok_or_else(overflow)?,
            )?;
        }
        Ok(())
    }
}
fn overflow() -> String {
    "compressed generic view storage overflow".into()
}
fn debug_len(value: &impl fmt::Debug) -> Result<usize, String> {
    struct Count(usize);
    impl fmt::Write for Count {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.0 = self.0.checked_add(value.len()).ok_or(fmt::Error)?;
            Ok(())
        }
    }
    let mut count = Count(0);
    fmt::write(&mut count, format_args!("{value:?}")).map_err(|_| overflow())?;
    Ok(count.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn formatting_census_counts_debug_escapes_without_building_strings() {
        let value = V::Text("quoted \"value\"\n".into());
        assert_eq!(debug_len(&value).unwrap(), format!("{value:?}").len());
        let mut budget = Budget(0);
        let length = property_value_bytes(&value, &mut budget).unwrap();
        assert_eq!(length, "quoted \"value\"\n".len());
        assert_eq!(budget.0, length);
    }
    #[test]
    fn complex_wording_constraints_refuse_instead_of_hiding_workspace() {
        let contract = conduit_core::CheckedValueContract {
            value_kind: conduit_core::kind_id("value/text"),
            maximum_bytes: 8,
            constraints: vec![ValueConstraint::TextPattern {
                pattern: conduit_core::CheckedTextPattern {
                    states: vec![],
                    start_state: 0,
                    maximum_input_characters: 8,
                    maximum_match_steps: 8,
                },
                anchored_start: true,
                anchored_end: true,
                negated: false,
            }],
        };
        assert!(simple_contract(&contract).is_err());
    }
    #[test]
    fn all_declared_series_owners_have_finite_separate_storage() {
        let mut one = Budget(0);
        quantity_series_storage(&mut one, 1).unwrap();
        let mut eight = Budget(0);
        quantity_series_storage(&mut eight, 8).unwrap();
        assert!(eight.0 > one.0);
        assert!(eight.0 < one.0 * 8); // one shared transient table set
        let mut overflowed = Budget(usize::MAX);
        assert!(overflowed.formatted(1).is_err());
    }
}
