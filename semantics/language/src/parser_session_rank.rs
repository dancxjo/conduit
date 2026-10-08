//! Finite observations for the original learned rank operation. The caller is
//! the closed Session driver: complete admitted Source/model histories establish
//! ancestry. This module supplies no grammar, legality, facts or commitments.
use crate::{
    parser_canonical_schema::{shape, Shape},
    parser_session_driver_profile::FixedParserDriverProfile,
    parser_session_numeric_profile::PinnedFourSlotNumericProfile,
};
use conduit_core::ValidatedCanonicalStructuredValue as Value;
use conduit_plot::rust_binding::PreparedNativeRustBinding;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DriverRankRefusal {
    Type,
    Field,
    Value,
    Shape,
}
pub(crate) fn field<'a>(
    mut value: Value<'a>,
    path: &[&str],
) -> Result<Value<'a>, DriverRankRefusal> {
    for name in path {
        value = value
            .record_field(name)
            .map_err(|_| DriverRankRefusal::Type)?
            .ok_or(DriverRankRefusal::Field)?;
    }
    Ok(value)
}
fn leaf<'a>(value: Value<'a>, identity: &str, width: usize) -> Result<&'a [u8], DriverRankRefusal> {
    if !matches!(shape(value.type_bytes()),Ok(Shape::Leaf(kind)) if kind==identity) {
        return Err(DriverRankRefusal::Type);
    }
    let node = value.value_node();
    if node.len() != width.checked_add(5).ok_or(DriverRankRefusal::Value)?
        || node.first() != Some(&0)
        || node.get(1..5) != Some((width as u32).to_le_bytes().as_slice())
    {
        return Err(DriverRankRefusal::Value);
    }
    Ok(&node[5..])
}
pub(crate) fn u64_value(value: Value<'_>) -> Result<u64, DriverRankRefusal> {
    Ok(u64::from_le_bytes(
        leaf(value, "value/u64", 8)?
            .try_into()
            .map_err(|_| DriverRankRefusal::Value)?,
    ))
}
pub(crate) fn i64_value(value: Value<'_>) -> Result<i64, DriverRankRefusal> {
    Ok(i64::from_le_bytes(
        leaf(value, "value/i64", 8)?
            .try_into()
            .map_err(|_| DriverRankRefusal::Value)?,
    ))
}
pub(crate) fn bool_value(value: Value<'_>) -> Result<bool, DriverRankRefusal> {
    match leaf(value, conduit_core::BOOL_INFO_ID, 1)? {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(DriverRankRefusal::Value),
    }
}
pub(crate) struct PreparedParserDriverRank<
    P: FixedParserDriverProfile = PinnedFourSlotNumericProfile,
> {
    profile: core::marker::PhantomData<P>,
    scores: [i64; 76],
    allowed: [bool; 76],
    classes: [usize; 4],
    length: usize,
}
impl<P: FixedParserDriverProfile> PreparedParserDriverRank<P> {
    /// Inline finite scratch is charged as part of the Session owner before its
    /// allocation. Runtime ranking neither allocates nor changes the legal mask.
    pub(crate) fn new() -> Self {
        Self {
            profile: core::marker::PhantomData,
            scores: [0; 76],
            allowed: [false; 76],
            classes: [0; 4],
            length: 0,
        }
    }
    pub(crate) fn rank(
        &mut self,
        scores: Value<'_>,
        mask: Value<'_>,
    ) -> Result<&[usize], DriverRankRefusal> {
        if P::SCORE_CLASSES != 76
            || scores.type_bytes() != P::Scores::PREPARED_DESCRIPTOR.type_bytes
            || mask.type_bytes() != P::Mask::PREPARED_DESCRIPTOR.type_bytes
        {
            return Err(DriverRankRefusal::Type);
        }
        let values = field(scores, &["scores"])?;
        let permissions = field(mask, &["allowed"])?;
        let values = values
            .collection_elements()
            .map_err(|_| DriverRankRefusal::Shape)?;
        let permissions = permissions
            .collection_elements()
            .map_err(|_| DriverRankRefusal::Shape)?;
        if values.len() != 76 || permissions.len() != 76 {
            return Err(DriverRankRefusal::Shape);
        }
        for (index, value) in values.enumerate() {
            self.scores[index] = i64_value(value.map_err(|_| DriverRankRefusal::Value)?)?;
        }
        for (index, value) in permissions.enumerate() {
            self.allowed[index] = bool_value(value.map_err(|_| DriverRankRefusal::Value)?)?;
        }
        self.length = conduit_ai::integer_masked_rank::integer_masked_top_k_into(
            &self.scores,
            &self.allowed,
            &mut self.classes,
        )
        .map_err(|_| DriverRankRefusal::Shape)?;
        Ok(&self.classes[..self.length])
    }
    pub(crate) fn selected(&self, index: usize) -> Option<(usize, i64)> {
        self.classes
            .get(..self.length)?
            .get(index)
            .map(|class| (*class, self.scores[*class]))
    }
}
