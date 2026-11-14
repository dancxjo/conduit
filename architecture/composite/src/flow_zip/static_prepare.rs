//! Explicit static workspace profile; original typed preparation gates remain.
use super::*;
impl FlowZipBack {
    /// Retain exact canonical member/output schemas in flash. The ordinary typed
    /// encoder is used transiently to preserve preparation checks; its peak is
    /// still part of the caller's preparation custody, not a claimed saving.
    pub fn prepare_typed_finite_static_workspace(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
        schemas: [&'static [u8]; 3],
    ) -> Result<Self, StructuredInfoRefusal> {
        let mut back =
            Self::prepare_typed_mode(left, left_type, right, right_type, true, Some(schemas))?;
        back.finite = true;
        Ok(back)
    }
    pub fn prepare_typed_feedback_static_workspace(
        left: &CheckedValueContract,
        left_type: StructuredInfoType,
        right: &CheckedValueContract,
        right_type: StructuredInfoType,
        schemas: [&'static [u8]; 3],
    ) -> Result<Self, StructuredInfoRefusal> {
        let mut back = Self::prepare_typed_finite_static_workspace(
            left, left_type, right, right_type, schemas,
        )?;
        back.feedback = true;
        back.return_owed = true;
        Ok(back)
    }
}
