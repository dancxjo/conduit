use super::SchedulerError;

pub(super) fn validate_active_capacity(
    active_nodes: usize,
    node_capacity: usize,
    active_cords: usize,
    cord_capacity: usize,
) -> Result<(), SchedulerError> {
    if active_nodes == 0 || active_nodes > node_capacity || active_cords > cord_capacity {
        return Err(SchedulerError::InvalidActiveCapacity);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_zero_cord_operations_but_not_zero_nodes_or_capacity_overflow() {
        assert_eq!(validate_active_capacity(1, 1, 0, 1), Ok(()));
        assert_eq!(
            validate_active_capacity(0, 1, 0, 1),
            Err(SchedulerError::InvalidActiveCapacity)
        );
        assert_eq!(
            validate_active_capacity(2, 1, 0, 1),
            Err(SchedulerError::InvalidActiveCapacity)
        );
        assert_eq!(
            validate_active_capacity(1, 1, 2, 1),
            Err(SchedulerError::InvalidActiveCapacity)
        );
    }
}
