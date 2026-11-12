//! Finite Source/Host-selected operation policy, never a received grant.
use super::{Operation, Refusal};
use conduit_resource_snapshot::{
    native_append as append, native_service as service, read_contract, service_contract,
};
/// One service node owns the residence. Its two independently scoped operations
/// use checked semantic targets, with no second physical resource or grant.
pub fn record_service_operations(
    nodes: &[(u16, &str)],
    maximum_records: Option<u32>,
    maximum_reads: u32,
) -> Result<Option<[Operation<'static>; 2]>, Refusal> {
    let mut selected = nodes
        .iter()
        .filter(|(_, kind)| *kind == service_contract::SERVICE_KIND);
    if selected.next().is_none() {
        return Ok(None);
    }
    if selected.next().is_some() || nodes.iter().any(|(_, kind)| *kind == append::APPEND_KIND) {
        return Err(Refusal::Scope);
    }
    let records = maximum_records.filter(|n| *n > 0).ok_or(Refusal::Bounds)?;
    if maximum_reads == 0 {
        return Err(Refusal::Bounds);
    }
    Ok(Some([
        Operation {
            kind: append::APPEND_KIND,
            authority: append::APPEND_AUTHORITY,
            call: append::APPEND_OPERATION,
            maximum_parameter_bytes: 200,
            maximum_result_bytes: append::APPEND_RESULT_BYTES,
            maximum_work_units: 6,
            maximum_operations: records,
        },
        Operation {
            kind: read_contract::READ_KIND,
            authority: service::READ_AUTHORITY,
            call: service::READ_OPERATION,
            maximum_parameter_bytes: read_contract::READ_INPUT_BYTES,
            maximum_result_bytes: conduit_resource_snapshot::read_result::READ_RESULT_BYTES as u32,
            maximum_work_units: read_contract::MAXIMUM_MEDIA_READS_PER_REQUEST,
            maximum_operations: maximum_reads,
        },
    ]))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn service_policy_keeps_both_original_targets_and_finite_source_bounds() {
        let nodes = [(7, service_contract::SERVICE_KIND)];
        let [append, read] = record_service_operations(&nodes, Some(17), 53)
            .unwrap()
            .unwrap();
        assert_eq!(
            (append.kind, append.maximum_operations),
            (append::APPEND_KIND, 17)
        );
        assert_eq!(
            (read.kind, read.maximum_operations),
            (read_contract::READ_KIND, 53)
        );
        assert_ne!(append.authority, read.authority);
        assert_ne!(append.call, read.call);
        assert!(record_service_operations(&nodes, None, 53).is_err());
        assert!(record_service_operations(&nodes, Some(17), 0).is_err());
        assert!(record_service_operations(
            &[
                (7, service_contract::SERVICE_KIND),
                (8, service_contract::SERVICE_KIND)
            ],
            Some(17),
            53
        )
        .is_err());
        assert!(record_service_operations(
            &[
                (7, service_contract::SERVICE_KIND),
                (8, append::APPEND_KIND)
            ],
            Some(17),
            53
        )
        .is_err());
        assert!(record_service_operations(&[], None, 0).unwrap().is_none());
    }
}
