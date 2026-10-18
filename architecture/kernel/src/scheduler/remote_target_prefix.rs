//! Streaming duplicate admission without rescanning an increasing CordId prefix.
use crate::{CordId, RemoteEndpointId};
#[derive(Default)]
pub(super) struct RemoteTargetPrefix {
    maximum_cord: Option<u16>,
    #[cfg(test)]
    comparisons: u64,
}
impl RemoteTargetPrefix {
    pub(super) fn contains(
        &mut self,
        prefix: &[(RemoteEndpointId, CordId)],
        target: (RemoteEndpointId, CordId),
    ) -> bool {
        let CordId(cord) = target.1;
        if self.maximum_cord.is_some_and(|maximum| cord <= maximum) {
            for previous in prefix {
                #[cfg(test)]
                {
                    self.comparisons += 1;
                }
                if *previous == target {
                    return true;
                }
            }
        }
        self.maximum_cord = Some(self.maximum_cord.map_or(cord, |maximum| maximum.max(cord)));
        false
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_results_match_tuple_prefix_oracle_for_arbitrary_order() {
        let choices = [
            (RemoteEndpointId(0), CordId(0)),
            (RemoteEndpointId(1), CordId(0)),
            (RemoteEndpointId(0), CordId(u16::MAX)),
            (RemoteEndpointId(1), CordId(1)),
        ];
        for length in 0..=8 {
            for mut code in 0..4usize.pow(length as u32) {
                let mut targets = [choices[0]; 8];
                for target in &mut targets[..length] {
                    *target = choices[code % 4];
                    code /= 4;
                }
                let mut seen = RemoteTargetPrefix::default();
                for index in 0..length {
                    assert_eq!(
                        seen.contains(&targets[..index], targets[index]),
                        targets[..index].contains(&targets[index])
                    );
                }
            }
        }
    }
    #[test]
    fn ten_thousand_increasing_targets_skip_all_prefix_comparisons() {
        let mut seen = RemoteTargetPrefix::default();
        let targets: [(RemoteEndpointId, CordId); 10_000] =
            core::array::from_fn(|i| (RemoteEndpointId(i as u16), CordId(i as u16)));
        for index in 0..targets.len() {
            assert!(!seen.contains(&targets[..index], targets[index]));
        }
        assert_eq!(seen.comparisons, 0);
        assert!(seen.contains(&targets, targets[9_999]));
        assert_eq!(seen.comparisons, 10_000);
    }
}
