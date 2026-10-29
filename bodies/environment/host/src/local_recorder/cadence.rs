//! Deadline selection only; this confers no transport or execution authority.
use std::time::Instant;
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Work {
    Collect,
    Observed,
    Network,
    Wait(Instant),
}
pub(super) fn select(
    now: Instant,
    collection: Instant,
    observed: Option<Instant>,
    network: Option<Instant>,
) -> Work {
    if now >= collection {
        return Work::Collect;
    }
    if observed.is_some_and(|due| now >= due) {
        return Work::Observed;
    }
    if network.is_some_and(|due| now >= due) {
        return Work::Network;
    }
    Work::Wait(
        [Some(collection), observed, network]
            .into_iter()
            .flatten()
            .min()
            .unwrap(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn collection_wins_simultaneous_deadlines_and_one_late_socket_operation() {
        let now = Instant::now();
        assert_eq!(select(now, now, Some(now), Some(now)), Work::Collect);
        assert_eq!(
            select(
                now + Duration::from_secs(3),
                now + Duration::from_secs(1),
                Some(now),
                Some(now)
            ),
            Work::Collect
        );
        assert_eq!(
            select(now, now + Duration::from_secs(30), Some(now), Some(now)),
            Work::Observed
        );
    }
    #[test]
    fn hundreds_of_pending_retries_cannot_starve_collection_or_observed_custody() {
        let start = Instant::now();
        let mut now = start;
        let (mut collection, mut observed, mut network) = (now, now, now);
        let (mut collections, mut observations, mut retries) = (0, 0, 0);
        while now < start + Duration::from_secs(300) {
            match select(now, collection, Some(observed), Some(network)) {
                Work::Collect => {
                    collections += 1;
                    collection = now + Duration::from_secs(30);
                }
                Work::Observed => {
                    observations += 1;
                    observed = now + Duration::from_secs(2);
                }
                Work::Network => {
                    retries += 1;
                    network = now + Duration::from_millis(400);
                }
                Work::Wait(until) => now = until,
            }
        }
        assert_eq!(collections, 10);
        assert_eq!(observations, 150);
        assert_eq!(retries, 750);
    }
    #[test]
    fn unavailable_custody_lanes_do_not_spin_or_delay_retrying_collection() {
        let now = Instant::now();
        let collection = now + Duration::from_secs(4);
        assert_eq!(select(now, collection, None, None), Work::Wait(collection));
        assert_eq!(select(now, collection, None, Some(now)), Work::Network);
    }
}
