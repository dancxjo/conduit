use super::*;

#[cfg(test)]
impl TourScheduler {
    pub(in crate::plot_runner) fn retained_text_snapshot(&self, node: NodeId) -> (u64, &[u8]) {
        self.text_states[usize::from(node.0)]
            .as_ref()
            .expect("prepared text state")
            .snapshot()
    }
}
