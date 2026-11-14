//! Retain actual nodes while keeping original logical envelopes and commit lengths.
use super::*;
impl FlowZipBack {
    pub(super) fn copy_static_input<const P: usize>(
        &mut self,
        inputs: &StepInputBytes<'_, P>,
        port: PortId,
        logical_length: usize,
        maximum: usize,
    ) -> Option<Result<usize, u16>> {
        let encoder = self.encoder.static_pair()?;
        let side = usize::from(port.0);
        Some((|| {
            let refusal = 915 + side as u16;
            if logical_length > maximum {
                return Err(refusal);
            }
            #[cfg(feature = "schema-value-storage")]
            let separated = inputs.canonical_parts(port);
            #[cfg(not(feature = "schema-value-storage"))]
            let separated: Option<conduit_kernel::CanonicalValueParts<'_>> = None;
            let node = if let Some(parts) = separated {
                #[cfg(feature = "schema-value-storage")]
                {
                    let schema = encoder.input_schema(side).ok_or(refusal)?;
                    if parts.schema != schema
                        || parts.schema.len().checked_add(parts.node.len()) != Some(logical_length)
                    {
                        return Err(refusal);
                    }
                    encoder
                        .validate_node(side, parts.node)
                        .map_err(|_| refusal)?;
                    parts.node
                }
                #[cfg(not(feature = "schema-value-storage"))]
                {
                    let _ = parts;
                    return Err(refusal);
                }
            } else {
                let bytes = inputs.input(port).ok_or(913 + side as u16)?;
                if bytes.len() != logical_length {
                    return Err(refusal);
                }
                encoder.input_node(side, bytes).map_err(|_| refusal)?
            };
            let slot = if side == 0 {
                &mut self.left
            } else {
                &mut self.right
            };
            if node.len() > slot.len() {
                return Err(refusal);
            }
            slot[..node.len()].copy_from_slice(node);
            Ok(logical_length)
        })())
    }
}
