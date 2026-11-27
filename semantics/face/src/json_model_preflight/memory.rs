//! Requested storage of the closed Face serde schema, before it is allocated.
//! This does not account for rendering, Plans, allocator metadata or RSS.
use super::JsonModelShape;
use crate::*;
use core::mem::size_of;

/// Bound serde's owned Face tables from an allocation-free JSON census.
/// The caller must refuse nonempty composition before deserialization; its
/// generated constructors have separate transient schema ownership.
/// Every dynamically sized sequence element is either an at-most-u64 scalar,
/// or one of these explicitly enumerated owning cell layouts. Their maximum bounds each cell. Vec growth is charged at twice the
/// number of elements plus four initial cells per array (including arrays
/// that ultimately deserialize into fixed-size numerical representations).
///
/// The only Box in this schema is a fixed Quantity in a ValueConstraint. Its
/// custom representation is an array; charge one box for every input array.
/// All owned strings plus serde's reusable escape buffer are charged at four
/// times their source spelling. No string can decode to more UTF-8 bytes than
/// its JSON source spelling. Overflow refuses admission.
pub fn face_model_preparation_bytes(shape: JsonModelShape) -> Option<usize> {
    let cell = [
        size_of::<FaceSubject>(),
        size_of::<FaceRelationship>(),
        size_of::<FaceCompositionRelation>(),
        size_of::<FaceProperty>(),
        size_of::<FaceText>(),
        size_of::<FaceAction>(),
        size_of::<FaceActionArgument>(),
        size_of::<FaceDisclosure>(),
        size_of::<TemporalReference>(),
        size_of::<FaceTemporalFact>(),
        size_of::<FaceContextBasis>(),
        size_of::<conduit_core::ValueConstraint>(),
        size_of::<conduit_core::TextPatternState>(),
        size_of::<conduit_core::TextPatternTransition>(),
        size_of::<alloc::vec::Vec<u8>>(),
        size_of::<alloc::string::String>(),
    ]
    .into_iter()
    .max()?;
    let cells = shape
        .object_or_string_array_elements
        .checked_mul(2)?
        .checked_add(shape.arrays.checked_mul(4)?)?;
    size_of::<Face>()
        .checked_add(20)?
        .checked_add(cells.checked_mul(cell)?)?
        .checked_add(
            shape
                .numeric_array_elements
                .checked_mul(2)?
                .checked_add(shape.arrays.checked_mul(8)?)?
                .checked_mul(size_of::<u64>())?,
        )?
        .checked_add(
            shape
                .arrays
                .checked_mul(size_of::<conduit_core::Quantity>())?,
        )?
        .checked_add(shape.string_source_bytes.checked_mul(4)?)
}
