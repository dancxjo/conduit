//! Affine custody for the existing per-exchange confined process.
//! Failed protocol/IO paths must quiesce and reap before releasing its resource.
pub(super) struct ProviderChild(std::process::Child);
impl ProviderChild {
    pub(super) fn new(child: std::process::Child) -> Self {
        Self(child)
    }
}
impl core::ops::Deref for ProviderChild {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl core::ops::DerefMut for ProviderChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for ProviderChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
