//! Exact physical segment checks; semantic/authority admission stays with Host.
use super::*;
pub(super) fn checked_parts<'a, S: ValueStorage>(
    store: &'a S,
    value: ValueRef,
    schema: Option<&[u8]>,
) -> Result<Option<crate::CanonicalValueParts<'a>>, SchedulerError> {
    let Some(parts) = store.canonical_parts(value)? else {
        return Ok(None);
    };
    if schema != Some(parts.schema)
        || parts
            .schema
            .len()
            .checked_add(parts.node.len())
            .and_then(|n| u32::try_from(n).ok())
            != Some(value.byte_len)
    {
        return Err(StorageError::RepresentationMismatch.into());
    }
    Ok(Some(parts))
}

pub(super) fn has_schema_profile<const P: usize, D: StepBack<P>>(drivers: &[D]) -> bool {
    drivers.iter().any(|driver| {
        (0..P).any(|port| {
            let port = PortId(port as u16);
            driver.canonical_input_schema(port).is_some()
                || driver.canonical_output_schema(port).is_some()
        })
    })
}
