//! Exact occurrence/head/base-relation vocative edge metrics.
#[derive(Default)]
pub struct VocativeEdges {
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
}
impl VocativeEdges {
    pub fn observe(&mut self, predicted: bool, reference: bool, same_head: bool) {
        let correct = predicted && reference && same_head;
        self.true_positive += usize::from(correct);
        self.false_positive += usize::from(predicted && !correct);
        self.false_negative += usize::from(reference && !correct);
    }
    pub fn precision(&self) -> Option<f64> {
        let denominator = self.true_positive + self.false_positive;
        (denominator != 0).then(|| self.true_positive as f64 / denominator as f64)
    }
    pub fn recall(&self) -> Option<f64> {
        let denominator = self.true_positive + self.false_negative;
        (denominator != 0).then(|| self.true_positive as f64 / denominator as f64)
    }
}

#[test]
fn wrong_head_counts_as_both_missed_and_spurious_vocative_edge() {
    let mut metric = VocativeEdges::default();
    assert_eq!(metric.precision(), None);
    assert_eq!(metric.recall(), None);
    metric.observe(true, true, true);
    metric.observe(true, true, false);
    metric.observe(true, false, true);
    metric.observe(false, true, true);
    metric.observe(false, false, true);
    assert_eq!(
        (
            metric.true_positive,
            metric.false_positive,
            metric.false_negative
        ),
        (1, 2, 2)
    );
    assert_eq!(metric.precision(), Some(1.0 / 3.0));
    assert_eq!(metric.recall(), Some(1.0 / 3.0));
}
