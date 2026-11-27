//! Borrowed preparation census for primitive and absent optional State seeds.
//! The ordinary decoder and validator still decide validity after admission.
use super::{canonical::Cursor, StructuredInfoType, StructuredInfoValue, StructuredVariantCase};
use core::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateInitializationLayout {
    /// Type/value decoding, including its temporary canonical self-check.
    pub decoded_peak_bytes: usize,
    /// Conservative schema ownership plus prefix, or one primitive Kind clone.
    pub validator_retained_bytes: usize,
    /// Both decoding and validator construction overlap within this ceiling.
    pub preparation_peak_bytes: usize,
}

impl StructuredInfoValue {
    /// No allocation. Qualifies one primitive leaf or the canonical `none`
    /// initialization of `T?` where T is a leaf. Other initializations remain
    /// outside this preparation profile, not outside the ordinary value codec.
    /// Excludes inline owners and caller-owned encoded bytes. Shared immutable
    /// schema aliases are charged per owner, conservatively as on non-atomic
    /// targets. This does not establish State, residence or effect authority.
    pub fn state_initialization_layout(encoded: &[u8]) -> Option<StateInitializationLayout> {
        if encoded.len() > super::MAXIMUM_STRUCTURED_CANONICAL_BYTES {
            return None;
        }
        let mut cursor = Cursor::new(encoded);
        let (type_bytes, child_kind, compound) = match cursor.byte().ok()? {
            0 => (cursor.text_ref().ok()?.len(), 0, false),
            3 => {
                let schema = cursor.text_ref().ok()?;
                if schema != "conduit.conduitese.optional.v1" || cursor.length().ok()? != 2 {
                    return None;
                }
                if cursor.text_ref().ok()? != "none" || cursor.byte().ok()? != 0 {
                    return None;
                }
                let absent_kind = cursor.text_ref().ok()?;
                if absent_kind != crate::EMPTY_INFO_ID
                    || cursor.text_ref().ok()? != "some"
                    || cursor.byte().ok()? != 0
                {
                    return None;
                }
                let present_kind = cursor.text_ref().ok()?;
                let type_bytes = schema
                    .len()
                    .checked_add(2usize.checked_mul(size_of::<StructuredVariantCase>())?)?
                    .checked_add(4 + 4)?
                    .checked_add(absent_kind.len())?
                    .checked_add(present_kind.len())?
                    .checked_add(StructuredInfoType::compound_backing_bytes())?;
                (type_bytes, absent_kind.len(), true)
            }
            _ => return None,
        };
        let prefix_bytes = encoded.len().checked_sub(cursor.remaining.len())?;
        let value_bytes = if compound {
            if cursor.byte().ok()? != 3
                || cursor.text_ref().ok()? != "none"
                || cursor.byte().ok()? != 0
                || !cursor.bytes().ok()?.is_empty()
            {
                return None;
            }
            size_of::<StructuredInfoValue>()
                .checked_add(4)?
                .checked_add(child_kind)?
        } else {
            if cursor.byte().ok()? != 0 {
                return None;
            }
            cursor.bytes().ok()?.len()
        };
        if !cursor.remaining.is_empty() {
            return None;
        }
        // decode_type owns one schema; decode_value clones it into its result.
        // Compound aliases may share on hosted targets, but paying both also
        // covers the non-atomic deep clone. The variant child's leaf clone is
        // counted separately with its box, tag and payload above.
        let decoded_peak_bytes = type_bytes
            .checked_mul(2)?
            .checked_add(value_bytes)?
            .checked_add(encoded.len())?;
        let validator_retained_bytes = if compound {
            type_bytes.checked_add(prefix_bytes)?
        } else {
            type_bytes
        };
        Some(StateInitializationLayout {
            decoded_peak_bytes,
            validator_retained_bytes,
            preparation_peak_bytes: decoded_peak_bytes.checked_add(validator_retained_bytes)?,
        })
    }
}
