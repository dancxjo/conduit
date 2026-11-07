//! Keep mutable local state and exported shared views on their original counter.
use super::*;
use conduit_std_host::body_execution::{ImageMemoryRetention, ImagePreparedValue};
use std::cell::UnsafeCell;

#[derive(Clone)]
pub(super) enum ChannelCustody {
    Reference(Arc<Channel>),
    Image(Arc<ImagePreparedValue<Channel>>),
}
impl ChannelCustody {
    pub(super) fn is_image(&self) -> bool {
        matches!(self, Self::Image(_))
    }
    pub(super) fn verify_image_profile(&self) -> Result<(), String> {
        if self.is_image() {
            Ok(())
        } else {
            Err("Crèche operation requires a funded IMAGE encounter".into())
        }
    }
    pub(super) fn verify_image(&self, host: &conduit_std_host::StdHost) -> Result<(), String> {
        match self {
            Self::Image(channel) => host.verify_selected_image_prepared_value(channel),
            Self::Reference(_) => Err("Crèche operation requires a funded IMAGE encounter".into()),
        }
    }
    pub(super) fn retain_memory(&self) -> Option<ImageMemoryRetention> {
        match self {
            Self::Reference(_) => None,
            Self::Image(channel) => Some(channel.retain_memory()),
        }
    }
    pub(super) fn retain_uncertain(&self) {
        if self.is_image() {
            // Last-resort drop cannot report quiescence. Keep shared Mask state
            // and its original charge alive until process recovery; never make
            // those bytes reusable while their effects remain uncertain.
            core::mem::forget(self.clone());
        }
    }
}
impl core::ops::Deref for ChannelCustody {
    type Target = Channel;
    fn deref(&self) -> &Channel {
        match self {
            Self::Reference(channel) => channel,
            Self::Image(channel) => channel.value(),
        }
    }
}

pub(super) enum LocalCustody<'a> {
    Reference(Local<'a>),
    Image(ImagePreparedValue<UnsafeCell<Local<'a>>>),
}
impl<'a> core::ops::Deref for LocalCustody<'a> {
    type Target = Local<'a>;
    fn deref(&self) -> &Local<'a> {
        match self {
            Self::Reference(local) => local,
            // SAFETY: this private wrapper uniquely owns its cell and never
            // exposes it. Shared access cannot overlap exclusive wrapper access.
            Self::Image(local) => unsafe { &*local.value().get() },
        }
    }
}
impl<'a> core::ops::DerefMut for LocalCustody<'a> {
    fn deref_mut(&mut self) -> &mut Local<'a> {
        match self {
            Self::Reference(local) => local,
            // SAFETY: exclusive access to this non-cloneable owner; no cell or
            // independent local reference escapes. Frame custody remains intact.
            Self::Image(local) => unsafe { &mut *local.value().get() },
        }
    }
}
