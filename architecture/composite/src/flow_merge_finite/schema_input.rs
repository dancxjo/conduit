//! Full input validation before ordinary reference transfer or schema fallback.
use super::*;
impl FlowMergeFiniteBack {
    #[cfg(feature = "schema-value-storage")]
    pub(super) fn linked_schema(&self) -> Option<&[u8]> {
        match self.validator.as_ref()? {
            Validator::Static(validator) => Some(validator.type_bytes()),
            Validator::Owned(_) => None,
        }
    }
}
pub(super) fn checked_input<'a, const P: usize>(
    validator: Option<&Validator>,
    leaf: Option<&KindId>,
    maximum_bytes: usize,
    inputs: &'a StepInputBytes<'_, P>,
    port: PortId,
    logical_length: usize,
) -> Result<Option<conduit_kernel::CanonicalValueParts<'a>>, u16> {
    if logical_length > maximum_bytes {
        return Err(1072);
    }
    #[cfg(feature = "schema-value-storage")]
    if let Some(parts) = inputs.canonical_parts(port) {
        let Some(Validator::Static(validator)) = validator else {
            return Err(1072);
        };
        if parts.schema != validator.type_bytes()
            || parts.schema.len().checked_add(parts.node.len()) != Some(logical_length)
            || validator.validate_node(parts.node).is_err()
        {
            return Err(1072);
        }
        return Ok(Some(parts));
    }
    let bytes = inputs.input(port).ok_or(1071u16)?;
    if bytes.len() != logical_length
        || validator.is_some_and(|v| v.validate(bytes).is_err())
        || leaf.is_some_and(|kind| {
            conduit_core::validate_primitive_info(kind.as_str(), bytes).is_err()
        })
    {
        return Err(1072);
    }
    Ok(None)
}
