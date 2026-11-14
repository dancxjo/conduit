//! Exact State input checks without changing held-reference custody.
use super::*;
impl SeededStateBack {
    #[cfg(all(feature = "execution-workspace", feature = "schema-value-storage"))]
    pub(super) fn linked_schema(&self) -> Option<&[u8]> {
        match &self.validator {
            Validator::Static(validator) => Some(validator.type_bytes()),
            _ => None,
        }
    }
    #[cfg(feature = "execution-workspace")]
    pub(super) fn state_workspace_requirements(
        &self,
    ) -> Option<conduit_kernel::scheduler::StepWorkspaceRequirements> {
        #[cfg(feature = "schema-value-storage")]
        if self.workspace_output_bytes != 0 {
            return Some(conduit_kernel::scheduler::StepWorkspaceRequirements {
                output_bytes: self.workspace_output_bytes,
                scratch_bytes: 0,
            });
        }
        None
    }
    pub(super) fn checked_input<'a, const P: usize>(
        &self,
        inputs: &'a StepInputBytes<'_, P>,
        port: PortId,
        logical_length: usize,
    ) -> Result<(bool, Option<conduit_kernel::CanonicalValueParts<'a>>), u16> {
        #[cfg(all(feature = "execution-workspace", feature = "schema-value-storage"))]
        if let Some(parts) = inputs.canonical_parts(port) {
            let Validator::Static(validator) = &self.validator else {
                return Err(934);
            };
            if parts.schema != validator.type_bytes()
                || parts.schema.len().checked_add(parts.node.len()) != Some(logical_length)
            {
                return Err(934);
            }
            let view = validator.validate_node(parts.node).map_err(|_| 934u16)?;
            return Ok((self.terminal_flag(view)?, Some(parts)));
        }
        let bytes = inputs.input(port).ok_or(933u16)?;
        if bytes.len() != logical_length || !self.valid(bytes) {
            return Err(934);
        }
        let terminal = if self.until {
            self.terminal_flag(
                conduit_core::validate_canonical_structured_value(bytes).map_err(|_| 935u16)?,
            )?
        } else {
            false
        };
        Ok((terminal, None))
    }
    fn terminal_flag(
        &self,
        view: conduit_core::ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<bool, u16> {
        if !self.until {
            return Ok(false);
        }
        let flag = view
            .record_field("terminal")
            .map_err(|_| 936u16)?
            .ok_or(936u16)?;
        let flag = flag
            .primitive_bytes(conduit_core::BOOL_INFO_ID)
            .map_err(|_| 937u16)?;
        Ok(flag == [1])
    }
}
