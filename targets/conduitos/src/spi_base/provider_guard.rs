//! Retained physical ownership enforces local safety on every preparation exit.
use super::SpiProvider;
pub(crate) struct ProviderGuard<P: SpiProvider>(Option<P>);
impl<P: SpiProvider> ProviderGuard<P> {
    pub(crate) fn into_inner(mut self) -> P {
        self.0.take().expect("retained native provider")
    }
    pub(crate) fn new(provider: P) -> Self {
        Self(Some(provider))
    }
}
impl<P: SpiProvider> core::ops::Deref for ProviderGuard<P> {
    type Target = P;
    fn deref(&self) -> &P {
        self.0.as_ref().expect("retained native provider")
    }
}
impl<P: SpiProvider> core::ops::DerefMut for ProviderGuard<P> {
    fn deref_mut(&mut self) -> &mut P {
        self.0.as_mut().expect("retained native provider")
    }
}
impl<P: SpiProvider> Drop for ProviderGuard<P> {
    fn drop(&mut self) {
        if let Some(provider) = self.0.as_mut() {
            provider.revoke();
        }
    }
}
