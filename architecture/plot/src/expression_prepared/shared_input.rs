//! Shared owned or installed immutable schema.
#[cfg(target_has_atomic = "ptr")]
type Owned = alloc::sync::Arc<[u8]>;
#[cfg(not(target_has_atomic = "ptr"))]
type Owned = alloc::rc::Rc<[u8]>;
#[derive(Clone)]
pub(super) enum SharedBytes {
    Owned(Owned),
    Static(&'static [u8]),
}
impl From<alloc::vec::Vec<u8>> for SharedBytes {
    fn from(v: alloc::vec::Vec<u8>) -> Self {
        Self::Owned(v.into())
    }
}
impl core::ops::Deref for SharedBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Owned(x) => x,
            Self::Static(x) => x,
        }
    }
}
impl AsRef<[u8]> for SharedBytes {
    fn as_ref(&self) -> &[u8] {
        self
    }
}
