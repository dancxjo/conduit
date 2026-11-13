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

impl<
        D: StepBack<P>,
        S: ValueStorage,
        E: SignSink,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
    > FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>
{
    /// Representation selection only. Every payload recipient must accept the
    /// exact immutable schema; remote/canonical boundaries retain full bytes.
    pub(super) fn prepared_output_uses_schema_storage(
        &self,
        node: usize,
        port: PortId,
    ) -> Result<bool, SchedulerError> {
        let Some(schema) = self.drivers[node].canonical_output_schema(port) else {
            return Ok(false);
        };
        if !self.values.supports_schema_storage(schema) {
            return Ok(false);
        }
        let mut payload = false;
        for target in self.routes.route(NodeId(as_u16(node)?), port)? {
            if self.cord_specs[usize::from(target.cord.0)].track != AssignedConnectionTrack::Payload
            {
                continue;
            }
            let CordEndpoint::Local { node, port } = target.sink else {
                return Ok(false);
            };
            if self.drivers[usize::from(node.0)].canonical_input_schema(port) != Some(schema) {
                return Ok(false);
            }
            payload = true;
        }
        Ok(payload)
    }
}
