//! One immutable input schema shared by all prepared expression children.
#[cfg(target_has_atomic = "ptr")]
pub(super) type SharedBytes = alloc::sync::Arc<[u8]>;
#[cfg(not(target_has_atomic = "ptr"))]
pub(super) type SharedBytes = alloc::rc::Rc<[u8]>;
