//! Same retained service Body; no second Host or independent biography writer.
use super::{DurableHostRuntime, HostSource};
use std::path::Path;
impl DurableHostRuntime {
    pub(crate) fn run_native_creche(&mut self, root: &Path, grants: &Path) -> Result<(), String> {
        if self.remote_fragment.is_some()
            || self.pool_member.is_some()
            || self.speech_worker.is_some()
            || self.owner_spoken_worker.is_some()
            || self.spoken_template_session.is_some()
        {
            return Err("native Crèche cannot replace an active installed operation".into());
        }
        match &mut self.host {
            HostSource::Body {
                owner,
                root: retained_root,
                running: None,
            } if retained_root == root => {
                crate::durable_host::native_creche::run(owner, root, grants)
            }
            _ => Err("native Crèche requires this retained idle Body owner".into()),
        }
    }
}
