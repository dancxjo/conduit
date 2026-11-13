//! Linked schema input validation; no value custody changes or alias fast path.
use super::*;
impl FlowMergeFiniteBack {
    #[cfg(feature = "schema-value-storage")]
    pub(super) fn linked_schema(&self) -> Option<&[u8]> {
        match self.validator.as_ref()? {
            Validator::Static(validator) => Some(validator.type_bytes()),
            Validator::Owned(_) => None,
        }
    }
    pub(super) fn copy_input<const P: usize>(
        &mut self,
        inputs: &StepInputBytes<'_, P>,
        port: PortId,
        logical_length: usize,
    ) -> Result<usize, u16> {
        #[cfg(feature = "schema-value-storage")]
        if let Some(parts) = inputs.canonical_parts(port) {
            let Some(Validator::Static(validator)) = self.validator.as_ref() else {
                return Err(1072);
            };
            let length = parts
                .schema
                .len()
                .checked_add(parts.node.len())
                .ok_or(1072u16)?;
            if parts.schema != validator.type_bytes()
                || length != logical_length
                || length > self.output.len()
                || validator.validate_node(parts.node).is_err()
            {
                return Err(1072);
            }
            let (schema, node) = self.output[..length].split_at_mut(parts.schema.len());
            schema.copy_from_slice(parts.schema);
            node.copy_from_slice(parts.node);
            return Ok(length);
        }
        let bytes = inputs.input(port).ok_or(1071u16)?;
        if bytes.len() != logical_length
            || bytes.len() > self.output.len()
            || self
                .validator
                .as_ref()
                .is_some_and(|v| v.validate(bytes).is_err())
            || self.leaf.as_ref().is_some_and(|kind| {
                conduit_core::validate_primitive_info(kind.as_str(), bytes).is_err()
            })
        {
            return Err(1072);
        }
        self.output[..bytes.len()].copy_from_slice(bytes);
        Ok(bytes.len())
    }
}
