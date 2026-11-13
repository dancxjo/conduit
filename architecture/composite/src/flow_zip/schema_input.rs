//! Reconstruct separated members in the existing admitted pair buffers.
use super::*;
impl FlowZipBack {
    pub(super) fn copy_input<const P: usize>(
        &mut self,
        inputs: &StepInputBytes<'_, P>,
        port: PortId,
        logical_length: usize,
        maximum: usize,
    ) -> Result<usize, u16> {
        let side = usize::from(port.0);
        #[cfg(feature = "schema-value-storage")]
        if let Some(parts) = inputs.canonical_parts(port) {
            let schema = self.encoder.input_schema(side).ok_or(915 + side as u16)?;
            let length = parts
                .schema
                .len()
                .checked_add(parts.node.len())
                .ok_or(915 + side as u16)?;
            if parts.schema != schema || length != logical_length || length > maximum {
                return Err(915 + side as u16);
            }
            conduit_core::BorrowedStructuredValueValidator::new(schema, maximum)
                .and_then(|validator| validator.validate_node(parts.node))
                .map_err(|_| 915 + side as u16)?;
            let slot = if side == 0 {
                &mut self.left
            } else {
                &mut self.right
            };
            slot[..schema.len()].copy_from_slice(schema);
            slot[schema.len()..length].copy_from_slice(parts.node);
            return Ok(length);
        }
        let bytes = inputs.input(port).ok_or(913 + side as u16)?;
        if bytes.len() != logical_length || bytes.len() > maximum {
            return Err(915 + side as u16);
        }
        let slot = if side == 0 {
            &mut self.left
        } else {
            &mut self.right
        };
        slot[..bytes.len()].copy_from_slice(bytes);
        Ok(bytes.len())
    }
}
