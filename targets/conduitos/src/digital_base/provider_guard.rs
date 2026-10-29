//! Retained physical ownership enforces local safety on every preparation exit.
use super::DigitalProvider;
pub(crate) struct ProviderGuard<P: DigitalProvider>(Option<P>);
impl<P: DigitalProvider> ProviderGuard<P> {
    pub(crate) fn into_inner(mut self) -> P {
        self.0.take().expect("retained native provider")
    }
    pub(crate) fn new(provider: P) -> Self {
        Self(Some(provider))
    }
}
impl<P: DigitalProvider> core::ops::Deref for ProviderGuard<P> {
    type Target = P;
    fn deref(&self) -> &P {
        self.0.as_ref().expect("retained native provider")
    }
}
impl<P: DigitalProvider> core::ops::DerefMut for ProviderGuard<P> {
    fn deref_mut(&mut self) -> &mut P {
        self.0.as_mut().expect("retained native provider")
    }
}
impl<P: DigitalProvider> Drop for ProviderGuard<P> {
    fn drop(&mut self) {
        if let Some(provider) = self.0.as_mut() {
            provider.revoke();
        }
    }
}
